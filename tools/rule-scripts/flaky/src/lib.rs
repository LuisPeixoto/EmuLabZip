//! Fails about a third of the requests, to see how the app copes. A script keeps nothing between calls, so the choice
//! comes from the URL: the same address always fails, or never does, which makes a problem easy to repeat.

use sdk::{answer, json};

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    sdk::alloc(len)
}

/// A small, stable hash (FNV-1a).
fn hash(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}

#[no_mangle]
pub extern "C" fn on_request(ptr: i32, len: i32) -> i64 {
    let call = sdk::input(ptr, len);
    let url = call["url"].as_str().unwrap_or("");
    if hash(url) % 3 == 0 {
        answer(&json!({
            "action": "replace",
            "status": 503,
            "headers": [["content-type", "application/json"], ["retry-after", "1"]],
            "text": "{\"error\":\"unavailable (made up by the flaky script)\"}",
            "message": "flaky: this address fails"
        }))
    } else {
        answer(&json!({ "action": "continue" }))
    }
}
