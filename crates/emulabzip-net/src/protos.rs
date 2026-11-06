//! Decoding gRPC messages with the app's `.proto` files.
//!
//! A folder of `.proto` files is compiled in memory (no `protoc` needed). Then the method in a call's path,
//! `/package.Service/Method`, says which message type the request and the response are, and the message is
//! decoded into the JSON protobuf defines for it, with the real field names.

use std::path::{Path, PathBuf};

use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use serde_json::Value;

use crate::grpc;

/// How many `.proto` files are read from a folder, so a wrong folder does not hang the app.
const MAX_FILES: usize = 2_000;

/// The messages and services compiled from a folder.
#[derive(Clone)]
pub struct ProtoSet {
    pool: DescriptorPool,
    files: usize,
    dir: PathBuf,
}

impl std::fmt::Debug for ProtoSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtoSet").field("dir", &self.dir).field("files", &self.files).finish()
    }
}

impl ProtoSet {
    /// Compiles every `.proto` under `dir`. Imports are looked up in `dir` too.
    pub fn load(dir: &Path) -> Result<ProtoSet, String> {
        let mut files = Vec::new();
        collect(dir, dir, &mut files, 0)?;
        if files.is_empty() {
            return Err(format!("There are no .proto files in {}.", dir.display()));
        }
        let count = files.len();
        let mut compiler = protox::Compiler::new([dir]).map_err(|e| e.to_string())?;
        compiler.include_imports(true);
        compiler.open_files(files).map_err(|e| e.to_string())?;
        Ok(ProtoSet { pool: compiler.descriptor_pool(), files: count, dir: dir.to_path_buf() })
    }

    pub fn file_count(&self) -> usize {
        self.files
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// How many services were found.
    pub fn service_count(&self) -> usize {
        self.pool.services().count()
    }

    /// The request and response types of `/package.Service/Method`.
    pub fn method(&self, path: &str) -> Option<(MessageDescriptor, MessageDescriptor)> {
        let path = path.split('?').next().unwrap_or(path);
        let mut parts = path.trim_matches('/').split('/');
        let (service, method) = (parts.next()?, parts.next()?);
        let method = self.pool.get_service_by_name(service)?.methods().find(|m| m.name() == method)?;
        Some((method.input(), method.output()))
    }

    /// The messages of a call body as JSON, one entry per message. `None` when the method is not in these files.
    /// A message that does not fit the type is shown as read without a schema.
    pub fn describe(&self, path: &str, request: bool, body: &[u8], encoding: Option<&str>) -> Option<Value> {
        let (input, output) = self.method(path)?;
        let ty = if request { input } else { output };
        let list = grpc::messages(body, encoding);
        Some(Value::Array(
            list.iter()
                .map(|m| {
                    if m.compressed {
                        return serde_json::json!({ "compressed": true, "bytes": m.data.len() });
                    }
                    decode(&ty, &m.data).unwrap_or_else(|| {
                        grpc::decode_raw(&m.data)
                            .unwrap_or_else(|| Value::String(format!("{} bytes, not protobuf", m.data.len())))
                    })
                })
                .collect(),
        ))
    }
}

fn decode(ty: &MessageDescriptor, data: &[u8]) -> Option<Value> {
    let message = DynamicMessage::decode(ty.clone(), data).ok()?;
    serde_json::to_value(&message).ok()
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<(), String> {
    if depth > 12 {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("Could not read {}: {e}", dir.display()))?;
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            collect(root, &path, out, depth + 1)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "proto") {
            if out.len() >= MAX_FILES {
                return Err(format!("More than {MAX_FILES} .proto files in {}.", root.display()));
            }
            // protox looks files up by their name inside the include folder.
            out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost_reflect::prost::Message;

    const PROTO: &str = r#"
syntax = "proto3";
package shop.v1;

message Item { string name = 1; int32 qty = 2; }
message Order { string id = 1; repeated Item items = 2; }
message Receipt { string id = 1; int64 total_cents = 2; }

service Shop { rpc Place(Order) returns (Receipt); }
"#;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("emulabzip-protos-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("shop")).unwrap();
        d
    }

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut m = vec![0u8];
        m.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        m.extend_from_slice(payload);
        m
    }

    #[test]
    fn decodes_a_call_with_the_field_names_of_the_proto() {
        let d = dir("ok");
        std::fs::write(d.join("shop/shop.proto"), PROTO).unwrap();
        let set = ProtoSet::load(&d).unwrap();
        assert_eq!((set.file_count(), set.service_count()), (1, 1));

        let (input, output) = set.method("/shop.v1.Shop/Place").unwrap();
        let order = DynamicMessage::deserialize(
            input,
            serde_json::json!({ "id": "o-1", "items": [{ "name": "pen", "qty": 3 }] }),
        )
        .unwrap();
        let body = frame(&order.encode_to_vec());
        let shown = set.describe("/shop.v1.Shop/Place", true, &body, None).unwrap();
        assert_eq!(shown, serde_json::json!([{ "id": "o-1", "items": [{ "name": "pen", "qty": 3 }] }]));

        let receipt =
            DynamicMessage::deserialize(output, serde_json::json!({ "id": "o-1", "totalCents": "450" })).unwrap();
        let shown = set.describe("/shop.v1.Shop/Place", false, &frame(&receipt.encode_to_vec()), None).unwrap();
        assert_eq!(shown, serde_json::json!([{ "id": "o-1", "totalCents": "450" }]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_method_that_is_not_in_the_files_is_left_to_the_raw_reader() {
        let d = dir("unknown");
        std::fs::write(d.join("shop/shop.proto"), PROTO).unwrap();
        let set = ProtoSet::load(&d).unwrap();
        assert!(set.method("/other.Service/Do").is_none());
        assert!(set.describe("/other.Service/Do", true, &frame(&[8, 1]), None).is_none());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_message_of_the_wrong_type_falls_back_to_the_raw_view() {
        let d = dir("wrong");
        std::fs::write(d.join("shop/shop.proto"), PROTO).unwrap();
        let set = ProtoSet::load(&d).unwrap();
        // Field 1 as a number, where Order has a string.
        let shown = set.describe("/shop.v1.Shop/Place", true, &frame(&[0x08, 5]), None).unwrap();
        assert_eq!(shown, serde_json::json!([{ "1": 5 }]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn says_what_is_wrong_with_the_folder() {
        let d = dir("bad");
        assert!(ProtoSet::load(&d).unwrap_err().contains("no .proto files"));
        std::fs::write(d.join("shop/bad.proto"), "syntax = \"proto3\"; message {").unwrap();
        assert!(ProtoSet::load(&d).is_err());
        assert!(ProtoSet::load(&d.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(d);
    }
}
