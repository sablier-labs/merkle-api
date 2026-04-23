use crate::utils::csv_validator::ValidationError;
use serde::Serialize;
use serde_json::Value as Json;
use vercel_runtime as Vercel;

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
}

/// Create a Bad Request type of response
pub fn bad_request(json_response: Json) -> R {
    R { status: 400, message: json_response }
}

/// Create a UNAUTHORIZED type of response
pub fn unauthorized(json_response: Json) -> R {
    R { status: 401, message: json_response }
}

/// Create a TOO_MANY_REQUESTS type of response
pub fn too_many_requests(json_response: Json) -> R {
    R { status: 429, message: json_response }
}

/// Create an Ok type of response
pub fn ok(json_response: Json) -> R {
    R { status: 200, message: json_response }
}

/// Create an Internal Server Error type of response
pub fn internal_server_error(json_response: Json) -> R {
    R { status: 500, message: json_response }
}

/// Converts a generic response in the format required by the Vercel serverless functions
pub fn to_vercel(response: R) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    Ok(Vercel::Response::builder()
        .status(response.status)
        .header("content-type", "application/json")
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, PATCH, PUT, DELETE, OPTIONS")
        .header("Access-Control-Allow-Headers", "Content-Type, Authorization")
        .body(response.message.to_string().into())?)
}
