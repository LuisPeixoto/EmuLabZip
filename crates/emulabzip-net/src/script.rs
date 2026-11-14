//! Rule scripts: small WebAssembly programs that look at a request or a response and decide what happens to it.
//!
//! A script is a `.wasm` file. It runs in [wasmi](https://github.com/wasmi-labs/wasmi), a WebAssembly interpreter, in a
//! fresh instance for every call, so one request can never see another. It has no files, no network and no clock: the
//! only function it can import is `env.log`. It is stopped after a fixed amount of work (fuel) and cannot grow past a
//! fixed amount of memory. A script runs at all only after its exact bytes have been approved (see [`Approvals`]).
//!
//! # The interface
//!
//! The module exports:
//! - `memory`: its linear memory;
//! - `alloc(len: i32) -> i32`: a pointer to `len` bytes the host may write into;
//! - `on_request(ptr: i32, len: i32) -> i64` and/or `on_response(ptr: i32, len: i32) -> i64`: given the JSON the host
//!   wrote at `ptr`, returns `(out_ptr << 32) | out_len`, where the answer, also JSON, is.
//!
//! The input is `{"method","url","headers":[["name","value"]],"body":"<base64>","bodyOmitted":false}` and, for a
//! response, also `"status"`. The answer is `{"action":"continue"|"replace"|"block"}` with, optionally, `"status"`,
//! `"headers"`, `"body"` (base64) or `"text"`, and `"message"`. `continue` with a field changes the request or the
//! response; `replace` (for a request) answers at once with that status, headers and body; `block` fails it.
//! The whole contract is in `docs/rule-scripts.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use base64::Engine as _;
use emulabzip_core::{Error, Result, data_dir};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmi::{Caller, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder};

use crate::flow::Headers;

/// How much work one call may do. Roughly one fuel per instruction: ten million takes a few milliseconds.
pub const FUEL: u64 = 10_000_000;
/// The biggest module, and the most memory a script may use.
pub const MAX_MODULE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_MEMORY: usize = 64 * 1024 * 1024;
/// A body bigger than this is not handed to the script.
pub const MAX_INPUT_BODY: usize = 1024 * 1024;
/// The most an answer may be.
pub const MAX_OUTPUT: usize = 8 * 1024 * 1024;
/// How much a script may write with `env.log`.
pub const MAX_LOG_LINES: usize = 20;
const MAX_LOG_LINE: usize = 1024;

/// What a script decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Continue,
    /// Answer now (for a request).
    Replace,
    Block,
}

/// What a script answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub verdict: Verdict,
    pub status: Option<u16>,
    pub headers: Option<Headers>,
    pub body: Option<Vec<u8>>,
    pub message: Option<String>,
    /// What the script wrote with `env.log`.
    pub logs: Vec<String>,
}

/// A request or a response as the script is shown it.
pub struct Call<'a> {
    pub method: &'a str,
    pub url: &'a str,
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
    /// Set for a response.
    pub status: Option<u16>,
}

struct Host {
    limits: StoreLimits,
    logs: Vec<String>,
}

/// A script that has been read and checked.
pub struct Script {
    engine: Engine,
    module: Module,
    linker: Linker<Host>,
    /// The SHA-256 of its bytes, in hex: what an approval is for.
    pub sha256: String,
    pub name: String,
    pub on_request: bool,
    pub on_response: bool,
}

impl std::fmt::Debug for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Script").field("name", &self.name).field("sha256", &self.sha256).finish()
    }
}

fn failed(what: impl std::fmt::Display) -> Error {
    Error::failed(what.to_string())
}

impl Script {
    /// Reads a script from bytes: it must be a valid module that only imports `env.log` and has the exports above.
    pub fn from_bytes(name: &str, bytes: &[u8]) -> Result<Script> {
        if bytes.len() as u64 > MAX_MODULE_BYTES {
            return Err(failed(format!("{name} is bigger than {} MB.", MAX_MODULE_BYTES / 1024 / 1024)));
        }
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes)
            .map_err(|e| failed(format!("{name} is not a valid WebAssembly module: {e}")))?;
        for import in module.imports() {
            if import.module() != "env" || import.name() != "log" {
                return Err(failed(format!(
                    "{name} asks for `{}.{}`, which a script is not given. The only function it can use is `env.log`.",
                    import.module(),
                    import.name()
                )));
            }
        }
        for needed in ["memory", "alloc"] {
            if module.get_export(needed).is_none() {
                return Err(failed(format!("{name} does not export `{needed}`.")));
            }
        }
        let (on_request, on_response) =
            (module.get_export("on_request").is_some(), module.get_export("on_response").is_some());
        if !on_request && !on_response {
            return Err(failed(format!("{name} exports neither `on_request` nor `on_response`.")));
        }
        let mut linker = <Linker<Host>>::new(&engine);
        linker
            .func_wrap("env", "log", |mut caller: Caller<'_, Host>, ptr: i32, len: i32| {
                let Some(memory) = caller.get_export("memory").and_then(wasmi::Extern::into_memory) else { return };
                let len = (len.max(0) as usize).min(MAX_LOG_LINE);
                let mut buffer = vec![0u8; len];
                if memory.read(&caller, ptr.max(0) as usize, &mut buffer).is_ok()
                    && caller.data().logs.len() < MAX_LOG_LINES
                {
                    caller.data_mut().logs.push(String::from_utf8_lossy(&buffer).into_owned());
                }
            })
            .map_err(failed)?;
        Ok(Script {
            engine,
            module,
            linker,
            sha256: hex(&Sha256::digest(bytes)),
            name: name.to_string(),
            on_request,
            on_response,
        })
    }

    /// Reads a script file.
    pub fn load(path: &Path) -> Result<Script> {
        let meta = std::fs::metadata(path).map_err(|e| failed(format!("Could not read {}: {e}", path.display())))?;
        if !meta.is_file() || meta.len() > MAX_MODULE_BYTES {
            return Err(failed(format!(
                "{} is not a file of at most {} MB.",
                path.display(),
                MAX_MODULE_BYTES / 1024 / 1024
            )));
        }
        let name = path.file_name().map_or_else(|| "script".to_string(), |n| n.to_string_lossy().into_owned());
        Script::from_bytes(&name, &std::fs::read(path)?)
    }

    /// Like [`Script::load`], and remembered until the file changes.
    pub fn cached(path: &Path) -> Result<Arc<Script>> {
        type Cache = Mutex<BTreeMap<PathBuf, (SystemTime, Arc<Script>)>>;
        static CACHE: OnceLock<Cache> = OnceLock::new();
        let cache = CACHE.get_or_init(Default::default);
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
        if let Some((at, script)) = cache.lock().unwrap().get(path)
            && *at == modified
        {
            return Ok(script.clone());
        }
        let script = Arc::new(Script::load(path)?);
        cache.lock().unwrap().insert(path.to_path_buf(), (modified, script.clone()));
        Ok(script)
    }

    /// Runs `on_request` for a request.
    pub fn request(&self, call: &Call<'_>) -> Result<Output> {
        self.run("on_request", call)
    }

    /// Runs `on_response` for a response.
    pub fn response(&self, call: &Call<'_>) -> Result<Output> {
        self.run("on_response", call)
    }

    fn run(&self, export: &str, call: &Call<'_>) -> Result<Output> {
        let mut input = json!({
            "method": call.method,
            "url": call.url,
            "headers": call.headers.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>(),
        });
        if call.body.len() > MAX_INPUT_BODY {
            input["bodyOmitted"] = json!(true);
            input["body"] = json!("");
        } else {
            input["bodyOmitted"] = json!(false);
            input["body"] = json!(base64::engine::general_purpose::STANDARD.encode(call.body));
        }
        if let Some(status) = call.status {
            input["status"] = json!(status);
        }
        let bytes = serde_json::to_vec(&input).map_err(failed)?;

        let mut store = Store::new(
            &self.engine,
            Host {
                limits: StoreLimitsBuilder::new()
                    .memory_size(MAX_MEMORY)
                    .table_elements(10_000)
                    .instances(2)
                    .memories(2)
                    .tables(2)
                    .build(),
                logs: Vec::new(),
            },
        );
        store.limiter(|host| &mut host.limits);
        store.set_fuel(FUEL).map_err(failed)?;
        let instance = self
            .linker
            .instantiate_and_start(&mut store, &self.module)
            .map_err(|e| failed(format!("{} could not start: {e}", self.name)))?;
        let memory: Memory =
            instance.get_memory(&store, "memory").ok_or_else(|| failed("The script has no memory."))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&store, "alloc")
            .map_err(|e| failed(format!("`alloc` does not have the shape (i32) -> i32: {e}")))?;
        let entry = instance
            .get_typed_func::<(i32, i32), i64>(&store, export)
            .map_err(|e| failed(format!("`{export}` does not have the shape (i32, i32) -> i64: {e}")))?;
        let trapped = |e: wasmi::Error| failed(format!("{} stopped: {e}", self.name));
        let at = alloc.call(&mut store, bytes.len() as i32).map_err(trapped)?;
        memory
            .write(&mut store, at as u32 as usize, &bytes)
            .map_err(|_| failed("`alloc` gave a pointer outside the script's memory."))?;
        let packed = entry.call(&mut store, (at, bytes.len() as i32)).map_err(trapped)?;
        let (out_ptr, out_len) = ((packed as u64 >> 32) as usize, (packed as u64 & 0xffff_ffff) as usize);
        if out_len > MAX_OUTPUT {
            return Err(failed(format!("{} answered with more than {} MB.", self.name, MAX_OUTPUT / 1024 / 1024)));
        }
        let mut out = vec![0u8; out_len];
        memory.read(&store, out_ptr, &mut out).map_err(|_| failed("The script's answer is outside its memory."))?;
        let logs = std::mem::take(&mut store.data_mut().logs);
        parse_output(&out, logs)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_headers(value: &Value) -> Result<Headers> {
    let list = value.as_array().ok_or_else(|| failed("`headers` must be a list of [name, value] pairs."))?;
    let mut out = Vec::with_capacity(list.len());
    for pair in list {
        let (Some(name), Some(text)) = (pair.get(0).and_then(Value::as_str), pair.get(1).and_then(Value::as_str))
        else {
            return Err(failed("`headers` must be a list of [name, value] pairs."));
        };
        if http::HeaderName::from_bytes(name.as_bytes()).is_err() || http::HeaderValue::from_str(text).is_err() {
            return Err(failed(format!("`{name}: {text}` is not a valid header.")));
        }
        out.push((name.to_string(), text.to_string()));
    }
    Ok(out)
}

/// Reads the JSON a script answered with.
fn parse_output(bytes: &[u8], logs: Vec<String>) -> Result<Output> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| failed(format!("The script's answer is not JSON: {e}")))?;
    let verdict = match value.get("action").and_then(Value::as_str) {
        Some("continue") | None => Verdict::Continue,
        Some("replace") => Verdict::Replace,
        Some("block") => Verdict::Block,
        Some(other) => return Err(failed(format!("`{other}` is not an action: use continue, replace or block."))),
    };
    let status = match value.get("status") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let code =
                v.as_u64().filter(|c| (100..=599).contains(c)).ok_or_else(|| failed("`status` must be 100 to 599."))?;
            Some(code as u16)
        }
    };
    let headers = match value.get("headers") {
        None | Some(Value::Null) => None,
        Some(v) => Some(parse_headers(v)?),
    };
    let body = match (value.get("body").and_then(Value::as_str), value.get("text").and_then(Value::as_str)) {
        (Some(b64), _) => Some(
            base64::engine::general_purpose::STANDARD
                .decode(b64)
                .map_err(|e| failed(format!("`body` is not base64: {e}")))?,
        ),
        (None, Some(text)) => Some(text.as_bytes().to_vec()),
        _ => None,
    };
    let message = value.get("message").and_then(Value::as_str).map(str::to_string);
    Ok(Output { verdict, status, headers, body, message, logs })
}

// ---------------------------------------------------------------------------------------------
// Approvals
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approved {
    pub sha256: String,
    pub name: String,
    pub at_ms: u64,
}

/// The scripts the user has allowed to run, by the SHA-256 of their bytes. Change one byte and the approval lapses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Approvals {
    list: Vec<Approved>,
    path: Option<PathBuf>,
}

impl Approvals {
    pub fn load() -> Approvals {
        Approvals::load_from(data_dir().join("approved-scripts.json"))
    }

    pub fn load_from(path: PathBuf) -> Approvals {
        let list = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Approvals { list, path: Some(path) }
    }

    pub fn is_approved(&self, sha256: &str) -> bool {
        self.list.iter().any(|a| a.sha256 == sha256)
    }

    pub fn list(&self) -> &[Approved] {
        &self.list
    }

    pub fn approve(&mut self, script: &Script) -> Result<()> {
        if !self.is_approved(&script.sha256) {
            let at_ms =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
            self.list.push(Approved { sha256: script.sha256.clone(), name: script.name.clone(), at_ms });
            self.save()?;
        }
        Ok(())
    }

    pub fn revoke(&mut self, sha256: &str) -> Result<()> {
        self.list.retain(|a| a.sha256 != sha256);
        self.save()
    }

    fn save(&self) -> Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(&self.list).map_err(failed)?)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }
}

/// Which scripts may run.
#[derive(Debug, Clone, Default)]
pub enum ApprovalPolicy {
    /// The scripts approved by the user, read from disk each time so an approval made in the app reaches a proxy that
    /// is already running.
    #[default]
    Saved,
    /// Only these (SHA-256, in hex). For tests and embedding.
    Only(Vec<String>),
}

impl ApprovalPolicy {
    pub fn allows(&self, sha256: &str) -> bool {
        match self {
            ApprovalPolicy::Saved => Approvals::load().is_approved(sha256),
            ApprovalPolicy::Only(list) => list.iter().any(|s| s == sha256),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A script written in the text format. `answer` is the JSON it always gives back, placed at offset 1024; the
    /// input is written at 4096 by `alloc`. `imports` and `extra` are added to the module.
    fn script(answer: &str, imports: &str, extra: &str) -> Vec<u8> {
        let escaped: String = answer.bytes().map(|b| format!("\\{b:02x}")).collect();
        let text = format!(
            r#"(module
                {imports}
                (memory (export "memory") 2)
                (data (i32.const 1024) "{escaped}")
                (func (export "alloc") (param i32) (result i32) (i32.const 4096))
                {extra}
                (func (export "on_request") (param i32 i32) (result i64)
                  (i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.const {len})))
                (func (export "on_response") (param i32 i32) (result i64)
                  (i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.const {len})))
            )"#,
            len = answer.len()
        );
        wat::parse_str(text).expect("the test module is valid")
    }

    fn call() -> Call<'static> {
        Call { method: "GET", url: "https://api.test/users?id=1", headers: &[], body: b"", status: None }
    }

    fn run(answer: &str) -> Result<Output> {
        Script::from_bytes("t.wasm", &script(answer, "", "")).unwrap().request(&call())
    }

    #[test]
    fn a_script_answers_with_what_it_decided() {
        let out =
            run(r#"{"action":"replace","status":418,"headers":[["x-by","script"]],"text":"short and stout"}"#).unwrap();
        assert_eq!(out.verdict, Verdict::Replace);
        assert_eq!(out.status, Some(418));
        assert_eq!(out.headers, Some(vec![("x-by".to_string(), "script".to_string())]));
        assert_eq!(out.body.as_deref(), Some(&b"short and stout"[..]));
        let block = run(r#"{"action":"block","message":"no thanks"}"#).unwrap();
        assert_eq!((block.verdict, block.message.as_deref()), (Verdict::Block, Some("no thanks")));
        let b64 = run(r#"{"body":"//4A"}"#).unwrap();
        assert_eq!((b64.verdict, b64.body.as_deref()), (Verdict::Continue, Some(&[0xff, 0xfe, 0x00][..])));
        assert_eq!(run("{}").unwrap().verdict, Verdict::Continue);
    }

    #[test]
    fn an_answer_that_is_not_valid_is_an_error_and_not_a_guess() {
        for bad in [
            "not json",
            r#"{"action":"explode"}"#,
            r#"{"status":99}"#,
            r#"{"headers":"x"}"#,
            r#"{"headers":[["bad name","v"]]}"#,
            r#"{"body":"***"}"#,
        ] {
            assert!(run(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_script_can_log_and_nothing_else() {
        let imports = r#"(import "env" "log" (func $log (param i32 i32)))"#;
        // The import is used from the start function, which runs when the module is instantiated.
        let text = format!(
            r#"(module {imports} (memory (export "memory") 1) (data (i32.const 0) "hi") (data (i32.const 1024) "{{}}")
               (func $start (call $log (i32.const 0) (i32.const 2)) (call $log (i32.const 0) (i32.const 2)))
               (start $start)
               (func (export "alloc") (param i32) (result i32) (i32.const 4096))
               (func (export "on_request") (param i32 i32) (result i64) (i64.or (i64.shl (i64.const 1024) (i64.const 32)) (i64.const 2))))"#
        );
        let out = Script::from_bytes("log.wasm", &wat::parse_str(text).unwrap()).unwrap().request(&call()).unwrap();
        assert_eq!(out.logs, ["hi", "hi"]);
        // A module that asks for anything else is refused before it can run.
        for import in [
            r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
            r#"(import "env" "fetch" (func (param i32)))"#,
        ] {
            let err = Script::from_bytes("x.wasm", &script("{}", import, "")).unwrap_err().to_string();
            assert!(err.contains("only function it can use is `env.log`"), "{err}");
        }
    }

    #[test]
    fn a_script_that_loops_forever_is_stopped_by_running_out_of_fuel() {
        let bytes = wat::parse_str(
            r#"(module (memory (export "memory") 1) (func (export "alloc") (param i32) (result i32) (i32.const 0))
                (func (export "on_request") (param i32 i32) (result i64) (loop $a (br $a)) (i64.const 0)))"#,
        )
        .unwrap();
        let err = Script::from_bytes("spin.wasm", &bytes).unwrap().request(&call()).unwrap_err().to_string();
        assert!(err.contains("stopped") && err.to_lowercase().contains("fuel"), "{err}");
    }

    #[test]
    fn a_script_cannot_use_more_memory_than_it_is_given() {
        let bytes = wat::parse_str(
            r#"(module (memory (export "memory") 1) (func (export "alloc") (param i32) (result i32) (i32.const 0))
                (func (export "on_request") (param i32 i32) (result i64)
                  (if (i32.ne (memory.grow (i32.const 2000)) (i32.const -1)) (then unreachable)) (i64.const 0)))"#,
        )
        .unwrap();
        // 2000 pages is 125 MB, past the limit: the grow fails with -1 and the script leaves cleanly (an empty answer).
        let result = Script::from_bytes("grow.wasm", &bytes).unwrap().request(&call());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not JSON"),
            "the grow was refused, so the script returned and its empty answer was read: {err}"
        );
    }

    #[test]
    fn a_trap_or_a_pointer_outside_memory_is_an_error() {
        let trap = wat::parse_str(
            r#"(module (memory (export "memory") 1) (func (export "alloc") (param i32) (result i32) (i32.const 0))
                (func (export "on_request") (param i32 i32) (result i64) unreachable))"#,
        )
        .unwrap();
        assert!(Script::from_bytes("t.wasm", &trap).unwrap().request(&call()).is_err());
        let outside = wat::parse_str(
            r#"(module (memory (export "memory") 1) (func (export "alloc") (param i32) (result i32) (i32.const 0))
                (func (export "on_request") (param i32 i32) (result i64) (i64.const 0x0000FFFF000000FF)))"#,
        )
        .unwrap();
        assert!(Script::from_bytes("o.wasm", &outside).unwrap().request(&call()).is_err());
    }

    #[test]
    fn a_module_must_have_the_exports_the_host_calls() {
        let no_entry = wat::parse_str(
            r#"(module (memory (export "memory") 1) (func (export "alloc") (param i32) (result i32) (i32.const 0)))"#,
        )
        .unwrap();
        assert!(Script::from_bytes("a.wasm", &no_entry).unwrap_err().to_string().contains("on_request"));
        let no_alloc = wat::parse_str(r#"(module (memory (export "memory") 1) (func (export "on_request") (param i32 i32) (result i64) (i64.const 0)))"#).unwrap();
        assert!(Script::from_bytes("b.wasm", &no_alloc).unwrap_err().to_string().contains("alloc"));
        assert!(Script::from_bytes("c.wasm", b"not wasm").unwrap_err().to_string().contains("not a valid WebAssembly"));
        assert!(Script::from_bytes("d.wasm", &vec![0u8; MAX_MODULE_BYTES as usize + 1]).is_err());
        let both = Script::from_bytes("e.wasm", &script("{}", "", "")).unwrap();
        assert!(both.on_request && both.on_response);
    }

    #[test]
    fn the_script_is_shown_the_request_as_json() {
        // The script echoes the input back as its `text`: the host reads at 4096 what it wrote, so a copy loop would do,
        // but it is enough to show that the input is JSON in the script's memory by letting it return a pointer to it.
        let bytes = wat::parse_str(
            r#"(module (memory (export "memory") 2) (func (export "alloc") (param i32) (result i32) (i32.const 4096))
                (func (export "on_request") (param $ptr i32) (param $len i32) (result i64)
                  (i64.or (i64.shl (i64.extend_i32_u (local.get $ptr)) (i64.const 32)) (i64.extend_i32_u (local.get $len)))))"#,
        )
        .unwrap();
        // The input is not an answer: it has no `action`, so it reads as `continue`, and it carries no fields we use.
        let out = Script::from_bytes("echo.wasm", &bytes)
            .unwrap()
            .request(&Call {
                method: "POST",
                url: "https://a.test/x",
                headers: &[("a".into(), "b".into())],
                body: b"hello",
                status: Some(200),
            })
            .unwrap();
        assert_eq!(out.verdict, Verdict::Continue);
        // `headers` in the input is a list of pairs, which is also how an answer says them, so it parses back.
        assert_eq!(out.headers, Some(vec![("a".to_string(), "b".to_string())]));
        // `body` in the input is base64 of "hello", which is also how an answer says it.
        assert_eq!(out.body.as_deref(), Some(&b"hello"[..]));
    }

    #[test]
    fn an_approval_is_for_the_exact_bytes() {
        let dir = std::env::temp_dir().join(format!("emulabzip-approvals-{}", std::process::id()));
        let path = dir.join("approved.json");
        let one = Script::from_bytes("one.wasm", &script("{}", "", "")).unwrap();
        let other = Script::from_bytes("two.wasm", &script(r#"{"action":"block"}"#, "", "")).unwrap();
        assert_ne!(one.sha256, other.sha256);
        assert_eq!(one.sha256.len(), 64);
        let mut approvals = Approvals::load_from(path.clone());
        assert!(!approvals.is_approved(&one.sha256));
        approvals.approve(&one).unwrap();
        approvals.approve(&one).unwrap();
        let again = Approvals::load_from(path.clone());
        assert!(again.is_approved(&one.sha256) && !again.is_approved(&other.sha256));
        assert_eq!(again.list().len(), 1);
        let mut revoked = again.clone();
        revoked.revoke(&one.sha256).unwrap();
        assert!(!Approvals::load_from(path).is_approved(&one.sha256));
        std::fs::remove_dir_all(dir).ok();
    }
}
