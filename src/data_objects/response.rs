use crate::utils::csv_validator::ValidationError;
use serde::Serialize;
use serde_json::{json, Value as Json};
use vercel_runtime as Vercel;

/// Eligibility results are deterministic per (cid, address) because CIDs are immutable.
/// Shipping this directive lets Vercel's edge cache serve repeat requests without
/// round-tripping to Pinata — it's our replacement for the Redis CID cache.
const IMMUTABLE_CACHE_CONTROL: &str = "public, s-maxage=31536000, immutable";

/// Generic Error Response structure
#[derive(Serialize, Debug)]
pub struct GeneralErrorResponse {
    pub message: String,
}

/// Struct for the response of the create endpoint when the provided csv is invalid
#[derive(Serialize, Debug)]
pub struct ValidationErrorResponse {
    pub status: String,
    pub errors: Vec<ValidationError>,
}

/// Struct for the success response of the create endpoint
#[derive(Serialize, Debug)]
pub struct UploadSuccessResponse {
    pub status: String,
    pub root: String,
    pub total: String,
    pub recipients: String,
    pub cid: String,
}

/// Struct for the success response of the eligibility endpoint
#[derive(Serialize, Debug)]
pub struct EligibilityResponse {
    pub index: usize,
    pub proof: Vec<String>,
    pub address: String,
    pub amount: String,
}

/// Struct for the success response of the validity endpoint
#[derive(Serialize, Debug)]
pub struct ValidResponse {
    pub root: String,
    pub total: String,
    pub recipients: String,
    pub cid: String,
}

/// Generic API response
#[derive(Serialize, Debug)]
pub struct R {
    pub status: u16,
    pub message: Json,
    #[serde(skip)]
    pub cache_control: Option<&'static str>,
}

/// Create a Bad Request type of response
pub fn bad_request(json_response: Json) -> R {
    R { status: 400, message: json_response, cache_control: None }
}

/// Create an Ok type of response
pub fn ok(json_response: Json) -> R {
    R { status: 200, message: json_response, cache_control: None }
}

/// Same as `ok`, but flags the response as immutably cacheable at Vercel's edge.
/// Use only for responses that are deterministic for a given URL (query string
/// included), such as eligibility results keyed by an immutable CID.
pub fn ok_immutable(json_response: Json) -> R {
    R { status: 200, message: json_response, cache_control: Some(IMMUTABLE_CACHE_CONTROL) }
}

/// Build a `GeneralErrorResponse`-shaped response with the given status and message.
pub fn message(status: u16, message: impl Into<String>) -> R {
    R {
        status,
        message: json!(GeneralErrorResponse { message: message.into() }),
        cache_control: None,
    }
}

/// Shorthand for `to_vercel(message(status, body))`, used by controllers to return
/// a Vercel-formatted `GeneralErrorResponse` in one call.
pub fn to_vercel_message(
    status: u16,
    body: impl Into<String>,
) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    to_vercel(message(status, body))
}

/// Converts a generic response in the format required by the Vercel serverless functions
pub fn to_vercel(response: R) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    let mut builder = Vercel::Response::builder()
        .status(response.status)
        .header("content-type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PATCH, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type, Authorization");

    if let Some(cc) = response.cache_control {
        builder = builder.header("Cache-Control", cc);
    }

    Ok(builder.body(response.message.to_string().into())?)
}
