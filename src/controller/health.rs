use crate::{data_objects::response, WebResult};
use serde::Serialize;
use serde_json::json;
use std::str;
use utoipa::ToSchema;

use vercel_runtime as Vercel;
use warp::Filter;

/// Health check response
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    /// Status of the server
    pub status: String,
    /// Health check message
    pub message: String,
}

/// Health request common handler. Returns an hardcoded message in order to display that the server works properly.
pub async fn handler() -> response::R {
    const MESSAGE: &str = "Server up and running";

    let result = json!({
        "status": "success".to_string(),
        "message": MESSAGE.to_string(),
    });

    response::ok(result)
}

/// Warp specific handler for the health endpoint
#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Server is healthy and running", body = HealthResponse)
    ),
    tag = "Health"
)]
pub async fn handler_to_warp() -> WebResult<impl warp::Reply> {
    let result = handler().await;
    Ok(response::to_warp(result))
}

/// Vercel specific handler for the health endpoint
pub async fn handler_to_vercel() -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    let result = handler().await;

    response::to_vercel(result)
}

/// Bind the route with the handler for the Warp handler.
pub fn build_route() -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "health").and(warp::get()).and_then(handler_to_warp)
}
