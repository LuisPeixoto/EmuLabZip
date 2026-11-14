//! What every rule script needs. The host calls `alloc` to get room for the request, then `on_request` or `on_response`
//! with where the request is; the script answers with a pointer and a length packed in one `i64`.

pub use base64;
pub use serde_json;
pub use serde_json::{json, Value};

/// Room for the host to write `len` bytes into. The memory is never freed: a script lives for one call.
pub fn alloc(len: i32) -> i32 {
    let buffer = vec![0u8; len.max(0) as usize];
    let ptr = buffer.as_ptr() as i32;
    std::mem::forget(buffer);
    ptr
}

/// The request or response the host wrote at `ptr`.
pub fn input(ptr: i32, len: i32) -> Value {
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}

/// Packs an answer for the host: its pointer in the high 32 bits and its length in the low 32.
pub fn answer(value: &Value) -> i64 {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|_| b"{}".to_vec());
    let (ptr, len) = (bytes.as_ptr() as i64, bytes.len() as i64);
    std::mem::forget(bytes);
    (ptr << 32) | len
}

/// A header's value, looked up without regard to case.
pub fn header<'a>(call: &'a Value, name: &str) -> Option<&'a str> {
    call["headers"].as_array()?.iter().find_map(|pair| {
        let key = pair.get(0)?.as_str()?;
        key.eq_ignore_ascii_case(name).then(|| pair.get(1)?.as_str()).flatten()
    })
}
