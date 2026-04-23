use crate::data_objects::response;
use serde_json::json;

use vercel_runtime as Vercel;

/// Health request common handler. Returns a hardcoded message to signal that the server is up.
pub async fn handler() -> response::R {
    response::ok(json!({
        "status": "success",
        "message": "Server up and running",
    }))
}

/// Vercel specific handler for the health endpoint
pub async fn handler_to_vercel() -> Result<Vercel::Response<Vercel::ResponseBody>, Vercel::Error> {
    let result = handler().await;

    response::to_vercel(result)
}
