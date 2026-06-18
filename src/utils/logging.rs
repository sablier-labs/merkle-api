use serde_json::{json, Map, Value as Json};
use sha3::{Digest, Keccak256};

/// Emits one-line JSON so Vercel can index app-level events.
pub fn log_event(level: &str, event: &str, fields: Json) {
    let mut object = match fields {
        Json::Object(map) => map,
        _ => Map::new(),
    };

    object.insert("level".to_string(), json!(level));
    object.insert("event".to_string(), json!(event));

    let line = Json::Object(object).to_string();
    if level == "error" {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

/// Hashes request identifiers that should be correlatable but not printed raw.
pub fn hash_identifier(value: &str) -> String {
    let digest = Keccak256::digest(value.as_bytes());
    hex::encode(&digest[..8])
}

/// Stable label for `reqwest::Error` without logging URLs or request headers.
pub fn reqwest_error_kind(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "request_timeout"
    } else if error.is_connect() {
        "request_connect"
    } else if error.is_decode() {
        "request_decode"
    } else if error.is_body() {
        "request_body"
    } else {
        "request_error"
    }
}
