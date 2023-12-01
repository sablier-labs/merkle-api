use crate::{
    csv_campaign_parser::CampaignCsvParsed,
    data_objects::{
        dto::{PersistentCampaignDto, RecipientDto},
        query_param::Create,
        response::{self, GeneralErrorResponse, UploadSuccessResponse, ValidationErrorResponse},
    },
    services::ipfs::{try_deserialize_pinata_response, upload_to_ipfs},
    FormData, StreamExt, TryStreamExt, WebResult,
};

use csv::ReaderBuilder;
use merkle_tree_rs::standard::StandardMerkleTree;
use std::{collections::HashMap, io::Read, num::ParseIntError, str};
use url::Url;

use serde_json::json;
use vercel_runtime as Vercel;
use warp::{Buf, Filter};

#[derive(Debug)]
struct CustomError(String);
impl warp::reject::Reject for CustomError {}

/// Create request common handler. It validates the received data, creates the merkle tree and uploads it to ipfs.
async fn handler(decimals: usize, buffer: &[u8]) -> response::R {
    let rdr = ReaderBuilder::new().from_reader(buffer);
    let parsed_csv = CampaignCsvParsed::build(rdr, decimals);

    if let Err(error) = parsed_csv {
        let response_json = json!(GeneralErrorResponse {
            message: format!("There was a problem in csv file parsing process: {}", error),
        });

        return response::internal_server_error(response_json);
    }

    let parsed_csv = parsed_csv.unwrap();
    if !parsed_csv.validation_errors.is_empty() {
        let response_json = json!(ValidationErrorResponse {
            status: String::from("Invalid csv file."),
            errors: parsed_csv.validation_errors,
        });

        return response::bad_request(response_json);
    }

    let leaves = parsed_csv
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| vec![i.to_string(), r.address.clone(), r.amount.to_string()])
        .collect();

    let tree = StandardMerkleTree::of(leaves, &["uint".to_string(), "address".to_string(), "uint256".to_string()]);

    let tree_json = serde_json::to_string(&tree.dump()).unwrap();

    let ipfs_response = upload_to_ipfs(PersistentCampaignDto {
        total_amount: parsed_csv.total_amount.to_string(),
        number_of_recipients: parsed_csv.number_of_recipients,
        merkle_tree: tree_json,
        root: tree.root(),
        recipients: parsed_csv
            .records
            .iter()
            .map(|x| RecipientDto { address: x.address.clone(), amount: x.amount.to_string() })
            .collect(),
    })
    .await;
    if ipfs_response.is_err() {
        let response_json =
            json!(GeneralErrorResponse { message: String::from("There was an error uploading the campaign to ipfs") });

        return response::internal_server_error(response_json);
    }

    let ipfs_response = ipfs_response.unwrap();
    let deserialized_response = try_deserialize_pinata_response(&ipfs_response);

    if deserialized_response.is_err() {
        let response_json =
            json!(GeneralErrorResponse { message: String::from("There was an error uploading the campaign to ipfs") });

        return response::internal_server_error(response_json);
    }

    let deserialized_response = deserialized_response.unwrap();

    let response_json = json!(UploadSuccessResponse {
        status: "Upload successful".to_string(),
        total: parsed_csv.total_amount.to_string(),
        recipients: parsed_csv.number_of_recipients.to_string(),
        root: tree.root(),
        cid: deserialized_response.ipfs_hash,
    });

    response::ok(response_json)
}

/// Warp specific handler for the create endpoint
pub async fn handler_to_warp(params: Create, form: FormData) -> WebResult<impl warp::Reply> {
    let decimals: Result<u16, ParseIntError> = params.decimals.parse();
    if decimals.is_err() {
        let response_json = json!(GeneralErrorResponse {
            message: String::from("Decimals query parameter is mandatory and should be a valid integer in order to create a valid campaign!"),
        });

        return Ok(response::to_warp(response::bad_request(response_json)));
    }
    let decimals = decimals.unwrap_or_default();
    let mut form = form;
    while let Some(Ok(part)) = form.next().await {
        let name = part.name();

        if name == "data" {
            let mut stream = part.stream();
            let mut buffer = Vec::new();

            while let Ok(Some(chunk)) = stream.try_next().await {
                chunk.reader().read_to_end(&mut buffer).unwrap();
            }

            let result = handler(decimals.into(), &buffer).await;
            return Ok(response::to_warp(result));
        }
    }

    let response_json = json!(GeneralErrorResponse {
        message: "The request form data did not contain recipients csv file".to_string()
    });
    Ok(response::to_warp(response::bad_request(response_json)))
}

/// Vercel specific handler for the create endpoint
pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    // ------------------------------------------------------------
    // Extract query parameters from the URL: decimals
    // ------------------------------------------------------------

    let url = Url::parse(&req.uri().to_string()).unwrap();
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let decimals = query.get("decimals");

    if decimals.is_none() {
        let response_json = json!(GeneralErrorResponse {
            message: String::from("Decimals query parameter is mandatory in order to create a valid campaign!"),
        });

        return response::to_vercel(response::ok(response_json));
    }

    // ------------------------------------------------------------
    // Extract form data from the body: file
    // ------------------------------------------------------------

    let boundary = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("multipart/form-data; boundary="));

    if boundary.is_none() {
        let response_json = json!(GeneralErrorResponse { message: String::from("Invalid content type header") });

        return response::to_vercel(response::ok(response_json));
    }

    let boundary = boundary.unwrap();
    let body = req.body().to_vec();

    let mut data = multipart::server::Multipart::with_body(body.as_slice(), boundary);
    let file = data.read_entry();
    if let Err(error) = file {
        let response_json = json!(GeneralErrorResponse { message: error.to_string() });

        return response::to_vercel(response::ok(response_json));
    }

    let file = file.unwrap();

    if file.is_none() {
        let response_json = json!(GeneralErrorResponse { message: String::from("Invalid form data, missing file") });

        return response::to_vercel(response::ok(response_json));
    }

    let mut file = file.unwrap();
    let mut buffer: Vec<u8> = vec![];

    if let Err(error) = file.data.read_to_end(&mut buffer) {
        let response_json = json!(GeneralErrorResponse { message: format!("Could not read body data {}", error) });

        return response::to_vercel(response::ok(response_json));
    }

    // ------------------------------------------------------------
    // Format arguments for the generic handler
    // ------------------------------------------------------------

    let decimals: Result<u16, ParseIntError> = decimals.unwrap().parse();
    if decimals.is_err() {
        let response_json = json!(GeneralErrorResponse {
            message: String::from("Decimals query parameter is mandatory and should be a valid integer in order to create a valid campaign!"),
        });

        return response::to_vercel(response::ok(response_json));
    }
    let decimals = decimals.unwrap_or_default();

    let result = handler(decimals.into(), &buffer).await;
    response::to_vercel(result)
}

/// Bind the route with the handler for the warp handler.
pub fn build_route() -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "create")
        .and(warp::post())
        .and(warp::query::query::<Create>())
        .and(warp::multipart::form().max_length(100_000_000))
        .and_then(handler_to_warp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::async_test::{setup_env_vars, SERVER};
    use warp::http::StatusCode;

    #[tokio::test]
    async fn test_valid_csv_upload() {
        let mut server = SERVER.lock().await;
        setup_env_vars(&server);
        let mock = server
            .mock("POST", "/pinning/pinFileToIPFS")
            .with_status(200)
            .with_body(r#"{"IpfsHash": "test_hash", "PinSize": 123, "Timestamp": "2021-01-01T00:00:00Z"}"#)
            .create();

        let csv_data = b"address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::OK.as_u16());
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_wrong_header() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount_invalid\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_missing_header() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);

        let csv_data =
            b"address\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_missing_column() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_invalid_address() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0xThisIsNotAnAddress,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_duplicated_addresses() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_invalid_amount() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);

        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,alphanumeric_amount\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_0() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_negative() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,-1\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_with_wrong_precision() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,1.1234\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, StatusCode::BAD_REQUEST.as_u16());
        drop(server);
    }
}
