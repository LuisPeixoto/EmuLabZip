//! Adds a header to every request and every response, to show that a script ran.

use sdk::{answer, json, Value};

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    sdk::alloc(len)
}

fn with_header(call: &Value, value: &str) -> i64 {
    let mut headers = call["headers"].as_array().cloned().unwrap_or_default();
    headers.push(json!(["x-emulabzip-script", value]));
    answer(&json!({ "action": "continue", "headers": headers }))
}

#[no_mangle]
pub extern "C" fn on_request(ptr: i32, len: i32) -> i64 {
    with_header(&sdk::input(ptr, len), "add-header: request")
}

#[no_mangle]
pub extern "C" fn on_response(ptr: i32, len: i32) -> i64 {
    with_header(&sdk::input(ptr, len), "add-header: response")
}
