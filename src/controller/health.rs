use crate::data_objects::response;
use serde_json::json;
use std::str;

use vercel_runtime as Vercel;

/// Health request common handler. Returns an hardcoded message in order to display that the server works properly.
pub async fn handler() -> response::R {
    const MESSAGE: &str = "Server up and running";

    let result = json!({
        "status": "success".to_string(),
        "message": MESSAGE.to_string(),
    });

    response::ok(result)
}

/// Vercel specific handler for the health endpoint
pub async fn handler_to_vercel() -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    let result = handler().await;

    response::to_vercel(result)
}
