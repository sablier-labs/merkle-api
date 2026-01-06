use crate::utils::csv_validator::ValidationError;
use serde::Serialize;
use serde_json::Value as Json;
use utoipa::ToSchema;
use vercel_runtime as Vercel;
use warp::reply::WithStatus;

/// Generic Error Response structure
#[derive(Serialize, Debug, ToSchema)]
pub struct GeneralErrorResponse {
    /// Error message describing what went wrong
    pub message: String,
}

/// Struct for the response of the create endpoint when the provided csv is invalid
#[derive(Serialize, Debug, ToSchema)]
pub struct ValidationErrorResponse {
    /// Status of the validation
    pub status: String,
    /// List of validation errors found in the CSV
    pub errors: Vec<ValidationError>,
}

/// Struct for the success response of the create endpoint
#[derive(Serialize, Debug, ToSchema)]
pub struct UploadSuccessResponse {
    /// Status message of the upload
    pub status: String,
    /// Merkle root hash of the campaign
    pub root: String,
    /// Total amount to be distributed
    pub total: String,
    /// Number of recipients in the campaign
    pub recipients: String,
    /// IPFS CID where the campaign data is stored
    pub cid: String,
}

/// Struct for the success response of the eligibility endpoint
#[derive(Serialize, Debug, ToSchema)]
pub struct EligibilityResponse {
    /// Index of the recipient in the merkle tree
    pub index: usize,
    /// Merkle proof for the recipient
    pub proof: Vec<String>,
    /// Address of the eligible recipient
    pub address: String,
    /// Amount the recipient is eligible to claim
    pub amount: String,
}

/// Struct for the success response of the validity endpoint
#[derive(Serialize, Debug, ToSchema)]
pub struct ValidResponse {
    /// Merkle root hash of the campaign
    pub root: String,
    /// Total amount to be distributed
    pub total: String,
    /// Number of recipients in the campaign
    pub recipients: String,
    /// IPFS CID of the campaign
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
    R { status: warp::http::StatusCode::BAD_REQUEST.as_u16(), message: json_response }
}

/// Create a UNAUTHORIZED type of response
pub fn unauthorized(json_response: Json) -> R {
    R { status: warp::http::StatusCode::UNAUTHORIZED.as_u16(), message: json_response }
}

/// Create an Ok type of response
pub fn ok(json_response: Json) -> R {
    R { status: warp::http::StatusCode::OK.as_u16(), message: json_response }
}

/// Create an Internal Server Error type of response
pub fn internal_server_error(json_response: Json) -> R {
    R { status: warp::http::StatusCode::INTERNAL_SERVER_ERROR.as_u16(), message: json_response }
}

/// Converts a generic response in the format required by Warp framework
pub fn to_warp(response: R) -> WithStatus<warp::reply::Json> {
    warp::reply::with_status(
        warp::reply::json(&response.message),
        warp::http::StatusCode::from_u16(response.status).unwrap(),
    )
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
