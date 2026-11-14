//! Hides the values of sensitive fields in JSON responses (`email`, `token`, `password`, `authorization`), so a
//! recording or a screenshot of the app does not carry them.

use sdk::base64::{engine::general_purpose::STANDARD, Engine};
use sdk::{answer, header, json, serde_json, Value};

const HIDDEN: [&str; 4] = ["email", "token", "password", "authorization"];

#[no_mangle]
pub extern "C" fn alloc(len: i32) -> i32 {
    sdk::alloc(len)
}

fn redact(value: &mut Value) -> usize {
    match value {
        Value::Object(map) => {
            let mut hidden = 0;
            for (key, v) in map.iter_mut() {
                if HIDDEN.iter().any(|h| key.eq_ignore_ascii_case(h)) && !v.is_null() {
                    *v = json!("***");
                    hidden += 1;
                } else {
                    hidden += redact(v);
                }
            }
            hidden
        }
        Value::Array(items) => items.iter_mut().map(redact).sum(),
        _ => 0,
    }
}

#[no_mangle]
pub extern "C" fn on_response(ptr: i32, len: i32) -> i64 {
    let call = sdk::input(ptr, len);
    let is_json = header(&call, "content-type").is_some_and(|t| t.contains("json"));
    let Some(body) = call["body"].as_str().and_then(|b| STANDARD.decode(b).ok()).filter(|_| is_json) else {
        return answer(&json!({ "action": "continue" }));
    };
    let Ok(mut parsed) = serde_json::from_slice::<Value>(&body) else {
        return answer(&json!({ "action": "continue" }));
    };
    if redact(&mut parsed) == 0 {
        return answer(&json!({ "action": "continue" }));
    }
    let text = serde_json::to_vec(&parsed).unwrap_or(body);
    answer(&json!({ "action": "continue", "body": STANDARD.encode(text) }))
}
