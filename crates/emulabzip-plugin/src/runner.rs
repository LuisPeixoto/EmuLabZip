//! Running a plugin: start the program for a device and turn its output into events.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use emulabzip_core::{Error, Platform, Result};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

use crate::event::{Event, parse_line};
use crate::manifest::{Mode, Plugin};

/// What a plugin gets to know: the device and, when one is picked, the app.
#[derive(Debug, Clone)]
pub struct Context {
    pub device: String,
    pub platform: Platform,
    pub app: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The program ended by itself, with this exit code.
    Finished(Option<i32>),
    /// It was stopped by the caller.
    Stopped,
    /// A `once` plugin ran longer than its timeout.
    TimedOut,
    /// It printed more than [`MAX_OUTPUT`] bytes.
    TooMuchOutput,
}

/// One line is at most this long; longer lines are skipped.
pub const MAX_LINE: u64 = 256 * 1024;
/// A run may print this much in total.
pub const MAX_OUTPUT: u64 = 8 * 1024 * 1024;
/// How long a `once` plugin may run when its manifest does not say.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_TIMEOUT_S: u64 = 600;
const STDERR_KEPT: usize = 2048;
/// After the program exits, how long its remaining output may take to arrive.
const DRAIN: Duration = Duration::from_millis(300);

/// The program to start: a file in the plugin's own folder wins over one on the PATH.
pub fn program(plugin: &Plugin) -> PathBuf {
    let name = &plugin.manifest.command[0];
    let local = plugin.dir.join(name);
    if local.exists() { local } else { PathBuf::from(name) }
}

/// Where `adb` is, for plugins that call it: the SDK's copy, or whatever the PATH has.
pub fn adb_path() -> PathBuf {
    let sdk_roots = [std::env::var_os("ANDROID_HOME"), std::env::var_os("ANDROID_SDK_ROOT")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .chain(std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Android/sdk")))
        .chain(std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Android/Sdk")))
        .chain(std::env::var_os("LOCALAPPDATA").map(|h| PathBuf::from(h).join("Android/Sdk")));
    let exe = if cfg!(windows) { "adb.exe" } else { "adb" };
    sdk_roots.map(|r| r.join("platform-tools").join(exe)).find(|p| p.is_file()).unwrap_or_else(|| PathBuf::from("adb"))
}

fn timeout_of(plugin: &Plugin) -> Option<Duration> {
    match plugin.manifest.mode {
        Mode::Stream => None,
        Mode::Once => {
            Some(plugin.manifest.timeout_s.map_or(DEFAULT_TIMEOUT, |s| Duration::from_secs(s.clamp(1, MAX_TIMEOUT_S))))
        }
    }
}

fn command(plugin: &Plugin, ctx: &Context) -> Command {
    let mut cmd = Command::new(program(plugin));
    cmd.args(&plugin.manifest.command[1..])
        .current_dir(&plugin.dir)
        .env("EMULABZIP_PROTOCOL", crate::PROTOCOL.to_string())
        .env("EMULABZIP_DEVICE", &ctx.device)
        .env("EMULABZIP_PLATFORM", format!("{:?}", ctx.platform).to_lowercase())
        .env("EMULABZIP_ADB", adb_path())
        .env("EMULABZIP_PLUGIN_DIR", &plugin.dir)
        .env_remove("EMULABZIP_APP")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(app) = &ctx.app {
        cmd.env("EMULABZIP_APP", app);
    }
    cmd
}

/// Starts the program. A program that was written a moment ago can be "busy" (`ETXTBSY`) while a
/// process forked at that instant still holds the file open for writing; it is free again within
/// milliseconds, so that one error is retried a few times.
async fn spawn(plugin: &Plugin, ctx: &Context) -> Result<tokio::process::Child> {
    let mut attempt = 0;
    loop {
        match command(plugin, ctx).spawn() {
            Ok(child) => return Ok(child),
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && attempt < 20 => {
                attempt += 1;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(Error::failed(format!("Could not start {}: {e}", plugin.manifest.command[0]))),
        }
    }
}

/// Reads one line of at most [`MAX_LINE`] bytes. `None` at the end of the stream; an over-long line
/// comes back empty after the rest of it has been thrown away.
async fn read_line<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R, bytes: &mut u64) -> Option<String> {
    let mut buf = Vec::new();
    let n = (&mut *reader).take(MAX_LINE).read_until(b'\n', &mut buf).await.ok()?;
    if n == 0 {
        return None;
    }
    *bytes += n as u64;
    if n as u64 == MAX_LINE && buf.last() != Some(&b'\n') {
        let mut rest = Vec::new();
        while let Ok(m) = reader.read_until(b'\n', &mut rest).await {
            *bytes += m as u64;
            if m == 0 || rest.last() == Some(&b'\n') || *bytes > MAX_OUTPUT {
                break;
            }
            rest.clear();
        }
        return Some(String::new());
    }
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// Runs a plugin and hands each event to `on_event` as it is printed. It returns when the program
/// ends, `stop` turns `true`, the timeout passes or the output limit is reached; the program is
/// killed in the last three cases.
///
/// A program that fails without saying why gets an [`Event::Error`] made from its exit code and the
/// end of its error output.
pub async fn run(
    plugin: &Plugin,
    ctx: &Context,
    mut stop: watch::Receiver<bool>,
    mut on_event: impl FnMut(Event) + Send,
) -> Result<Outcome> {
    let mut child = spawn(plugin, ctx).await?;
    let mut lines = BufReader::new(child.stdout.take().expect("stdout is piped"));
    let mut stderr = child.stderr.take().expect("stderr is piped");
    // What the program wrote on standard error, kept as it arrives: the stream may stay open after
    // the program is gone, and what was read by then is still worth showing.
    let kept = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    let sink = kept.clone();
    let errors = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        while let Ok(n) = stderr.read(&mut buf).await {
            if n == 0 {
                break;
            }
            if let Ok(mut kept) = sink.lock() {
                kept.extend_from_slice(&buf[..n]);
                let over = kept.len().saturating_sub(STDERR_KEPT);
                kept.drain(..over);
            }
        }
    });

    let deadline = timeout_of(plugin).map(|t| tokio::time::Instant::now() + t);
    let mut bytes = 0u64;
    let mut said_error = false;
    // The program's exit code once it has exited, and the moment the output it left behind has
    // had long enough to arrive. The end of the output is not the end of the program: a program
    // that starts something in the background hands it the pipe, and the pipe stays open.
    let mut exited: Option<Option<i32>> = None;
    let mut drain_until: Option<tokio::time::Instant> = None;
    let outcome = loop {
        let timeout = async {
            match deadline {
                Some(d) => tokio::time::sleep_until(d).await,
                None => std::future::pending().await,
            }
        };
        let drained = async {
            match drain_until {
                Some(d) => tokio::time::sleep_until(d).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            line = read_line(&mut lines, &mut bytes) => match line {
                Some(line) => {
                    if let Some(event) = parse_line(&line) {
                        said_error |= matches!(event, Event::Error { .. });
                        on_event(event);
                    }
                    if bytes > MAX_OUTPUT {
                        break Outcome::TooMuchOutput;
                    }
                }
                None => {
                    let code = match exited {
                        Some(code) => code,
                        None => child.wait().await.ok().and_then(|s| s.code()),
                    };
                    break Outcome::Finished(code);
                }
            },
            status = child.wait(), if exited.is_none() => {
                exited = Some(status.ok().and_then(|s| s.code()));
                drain_until = Some(tokio::time::Instant::now() + DRAIN);
            }
            _ = drained => break Outcome::Finished(exited.flatten()),
            _ = timeout => break Outcome::TimedOut,
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break Outcome::Stopped;
                }
            }
        }
    };
    if !matches!(outcome, Outcome::Finished(_)) {
        let _ = child.kill().await;
    }
    let _ = tokio::time::timeout(DRAIN, errors).await;
    let tail = kept.lock().map(|k| String::from_utf8_lossy(&k).trim().to_string()).unwrap_or_default();
    match outcome {
        Outcome::Finished(code) if code != Some(0) && !said_error => {
            let code = code.map_or("a signal".to_string(), |c| format!("code {c}"));
            let why = if tail.is_empty() { String::new() } else { format!(": {tail}") };
            on_event(Event::Error { message: format!("The plugin stopped with {code}{why}") });
        }
        Outcome::TimedOut => on_event(Event::Error {
            message: format!(
                "The plugin did not finish in {} seconds and was stopped",
                timeout_of(plugin).map_or(0, |t| t.as_secs())
            ),
        }),
        Outcome::TooMuchOutput => {
            on_event(Event::Error { message: "The plugin printed too much and was stopped".into() })
        }
        _ => {}
    }
    Ok(outcome)
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;
    use crate::manifest::{Manifest, load};

    fn plugin(name: &str, mode: &str, script: &str, extra: &str) -> (Plugin, PathBuf) {
        let root = std::env::temp_dir().join(format!("emulabzip-run-{name}-{}", std::process::id()));
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("run.sh"), script).unwrap();
        std::fs::set_permissions(dir.join("run.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
        let manifest = format!(
            r#"{{ "protocol": 1, "name": "{name}", "version": "1", "command": ["./run.sh"], "mode": "{mode}" {extra} }}"#
        );
        Manifest::parse(&manifest).unwrap();
        std::fs::write(dir.join("plugin.json"), manifest).unwrap();
        (load(&dir).unwrap(), root)
    }

    fn ctx() -> Context {
        Context { device: "emulator-5554".into(), platform: Platform::Android, app: Some("com.example.app".into()) }
    }

    async fn collect(plugin: &Plugin, ctx: &Context) -> (Outcome, Vec<Event>) {
        let (_keep, stop) = watch::channel(false);
        let mut events = Vec::new();
        let outcome = run(plugin, ctx, stop, |e| events.push(e)).await.unwrap();
        (outcome, events)
    }

    #[tokio::test]
    async fn runs_a_program_and_reads_what_it_prints() {
        let script = "#!/bin/sh\necho 'starting up'\necho '{\"type\":\"metric\",\"name\":\"n\",\"value\":2}'\necho '{\"type\":\"video\"}'\necho '{\"type\":\"text\",\"body\":\"done\"}'\n";
        let (p, root) = plugin("prints", "once", script, "");
        let (outcome, events) = collect(&p, &ctx()).await;
        assert_eq!(outcome, Outcome::Finished(Some(0)));
        assert_eq!(events.len(), 2, "the progress line and the unknown event are skipped: {events:?}");
        assert!(matches!(events[0], Event::Metric { value, .. } if value == 2.0));
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn tells_the_program_about_the_device_and_the_app() {
        let script = "#!/bin/sh\nprintf '{\"type\":\"text\",\"body\":\"%s|%s|%s|%s|%s\"}\\n' \"$EMULABZIP_DEVICE\" \"$EMULABZIP_PLATFORM\" \"$EMULABZIP_APP\" \"$EMULABZIP_PROTOCOL\" \"$(basename \"$PWD\")\"\n";
        let (p, root) = plugin("env", "once", script, "");
        let (_, events) = collect(&p, &ctx()).await;
        assert!(
            matches!(&events[0], Event::Text { body, .. } if body == "emulator-5554|android|com.example.app|1|env"),
            "{events:?}"
        );
        let (_, events) = collect(&p, &Context { app: None, ..ctx() }).await;
        assert!(
            matches!(&events[0], Event::Text { body, .. } if body.contains("android||1")),
            "no app means an empty variable: {events:?}"
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_failing_program_explains_itself() {
        let (p, root) = plugin("fails", "once", "#!/bin/sh\necho 'adb: device not found' >&2\nexit 3\n", "");
        let (outcome, events) = collect(&p, &ctx()).await;
        assert_eq!(outcome, Outcome::Finished(Some(3)));
        assert!(
            matches!(&events[0], Event::Error { message } if message.contains("code 3") && message.contains("device not found")),
            "{events:?}"
        );
        // One that already reported its own error is not told on twice.
        let (p, root2) =
            plugin("says", "once", "#!/bin/sh\necho '{\"type\":\"error\",\"message\":\"mine\"}'\nexit 1\n", "");
        let (_, events) = collect(&p, &ctx()).await;
        assert_eq!(events, [Event::Error { message: "mine".into() }]);
        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(root2).ok();
    }

    #[tokio::test]
    async fn stops_a_program_that_takes_too_long() {
        let (p, root) = plugin("slow", "once", "#!/bin/sh\nsleep 30\n", ", \"timeoutS\": 1");
        let started = std::time::Instant::now();
        let (outcome, events) = collect(&p, &ctx()).await;
        assert_eq!(outcome, Outcome::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(matches!(&events[0], Event::Error { message } if message.contains("1 seconds")), "{events:?}");
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_stream_runs_until_it_is_stopped() {
        let script =
            "#!/bin/sh\nwhile true; do echo '{\"type\":\"metric\",\"name\":\"t\",\"value\":1}'; sleep 0.1; done\n";
        let (p, root) = plugin("stream", "stream", script, "");
        let (stop, stopped) = watch::channel(false);
        let mut seen = 0;
        let ctx = ctx();
        let runner = tokio::spawn({
            let p = p.clone();
            async move {
                let mut count = 0;
                let outcome = run(&p, &ctx, stopped, |_| count += 1).await.unwrap();
                (outcome, count)
            }
        });
        tokio::time::sleep(Duration::from_millis(700)).await;
        stop.send(true).unwrap();
        let (outcome, count) = runner.await.unwrap();
        seen += count;
        assert_eq!(outcome, Outcome::Stopped);
        assert!(seen >= 3, "{seen} events before it was stopped");
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn skips_a_line_that_is_too_long_and_stops_a_flood() {
        let script =
            "#!/bin/sh\nhead -c 400000 /dev/zero | tr '\\0' 'a'\necho\necho '{\"type\":\"text\",\"body\":\"after\"}'\n";
        let (p, root) = plugin("long", "once", script, "");
        let (outcome, events) = collect(&p, &ctx()).await;
        assert_eq!(outcome, Outcome::Finished(Some(0)));
        assert_eq!(
            events,
            [Event::Text { title: None, body: "after".into() }],
            "the long line is dropped, the next one is read"
        );
        let (p, root2) = plugin("flood", "once", "#!/bin/sh\nyes '{\"type\":\"log\",\"message\":\"x\"}'\n", "");
        let (outcome, events) = collect(&p, &ctx()).await;
        assert_eq!(outcome, Outcome::TooMuchOutput);
        assert!(matches!(events.last(), Some(Event::Error { .. })));
        std::fs::remove_dir_all(root).ok();
        std::fs::remove_dir_all(root2).ok();
    }

    #[tokio::test]
    async fn does_not_wait_for_a_background_process_that_keeps_the_output_open() {
        let script = "#!/bin/sh\necho '{\"type\":\"text\",\"body\":\"before\"}'\n(sleep 20) &\necho '{\"type\":\"text\",\"body\":\"after\"}'\n";
        let (p, root) = plugin("background", "once", script, "");
        let started = std::time::Instant::now();
        let (outcome, events) = collect(&p, &ctx()).await;
        assert_eq!(outcome, Outcome::Finished(Some(0)));
        assert_eq!(events.len(), 2, "both lines are read: {events:?}");
        assert!(started.elapsed() < Duration::from_secs(5), "waited {:?}", started.elapsed());
        std::fs::remove_dir_all(root).ok();
    }

    #[tokio::test]
    async fn a_missing_program_is_an_error() {
        let (mut p, root) = plugin("missing", "once", "#!/bin/sh\n", "");
        p.manifest.command = vec!["definitely-not-a-program".into()];
        let (_keep, stop) = watch::channel(false);
        let err = run(&p, &ctx(), stop, |_| {}).await.unwrap_err();
        assert!(err.to_string().contains("Could not start definitely-not-a-program"), "{err}");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn a_file_in_the_plugin_folder_wins_over_the_path() {
        let (p, root) = plugin("local", "once", "#!/bin/sh\n", "");
        assert_eq!(program(&p), p.dir.join("./run.sh"));
        assert!(program(&p).is_file());
        std::fs::remove_dir_all(root).ok();
    }
}
