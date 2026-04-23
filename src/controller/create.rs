use crate::{
    csv_campaign_parser::CampaignCsvParsed,
    data_objects::{
        dto::{PersistentCampaignDto, RecipientDto},
        response::{self, UploadSuccessResponse, ValidationErrorResponse},
    },
    services::ipfs::{try_deserialize_pinata_response, upload_to_ipfs},
    utils::{auth, request},
};

use csv::ReaderBuilder;
use http_body_util::BodyExt;
use merkle_tree_rs::standard::StandardMerkleTree;
use std::io::Read;

use serde_json::json;
use vercel_runtime as Vercel;

/// Create request common handler. It validates the received data, creates the merkle tree and uploads it to ipfs.
async fn handler(decimals: usize, buffer: &[u8]) -> response::R {
    let rdr = ReaderBuilder::new().from_reader(buffer);
    let parsed_csv = match CampaignCsvParsed::build_ethereum(rdr, decimals) {
        Ok(parsed) => parsed,
        Err(error) => {
            return response::message(500, format!("There was a problem in csv file parsing process: {error}"));
        }
    };

    if !parsed_csv.validation_errors.is_empty() {
        let response_json = json!(ValidationErrorResponse {
            status: "Invalid csv file.".to_string(),
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

    let dto = PersistentCampaignDto {
        total_amount: parsed_csv.total_amount.to_string(),
        number_of_recipients: parsed_csv.number_of_recipients,
        merkle_tree: tree_json,
        root: tree.root(),
        recipients: parsed_csv
            .records
            .iter()
            .map(|x| RecipientDto { address: x.address.clone(), amount: x.amount.to_string() })
            .collect(),
    };

    let ipfs_response = match upload_to_ipfs(&dto).await {
        Ok(response) => response,
        Err(error) => {
            println!("Error: {error}");
            return response::message(500, "There was an error uploading the campaign to ipfs");
        }
    };

    let deserialized_response = match try_deserialize_pinata_response(&ipfs_response) {
        Ok(response) => response,
        Err(error) => {
            println!("Error: {error}");
            return response::message(500, "There was an error uploading the campaign to ipfs");
        }
    };

    let response_json = json!(UploadSuccessResponse {
        status: "Upload successful".to_string(),
        total: parsed_csv.total_amount.to_string(),
        recipients: parsed_csv.number_of_recipients.to_string(),
        root: tree.root(),
        cid: deserialized_response.ipfs_hash,
    });

    response::ok(response_json)
}

/// Vercel specific handler for the create endpoint
pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::ResponseBody>, Vercel::Error> {
    if !auth::is_authorized(&req) {
        return response::to_vercel_message(401, "Bad authentication process provided.");
    }

    // ------------------------------------------------------------
    // Extract query parameters from the URL: decimals
    //
    // NOTE: the missing/malformed-input branches below intentionally return status 200
    // to preserve legacy client behavior. Review candidate.
    // ------------------------------------------------------------

    let query = request::query_params(&req);
    let Some(decimals) = query.get("decimals").cloned() else {
        return response::to_vercel_message(
            200,
            "Decimals query parameter is mandatory in order to create a valid campaign!",
        );
    };

    // ------------------------------------------------------------
    // Extract form data from the body: file
    // ------------------------------------------------------------

    let Some(boundary) = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("multipart/form-data; boundary="))
        .map(String::from)
    else {
        return response::to_vercel_message(200, "Invalid content type header");
    };

    let body = match req.into_body().collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(error) => return response::to_vercel_message(200, format!("Could not read body data {error}")),
    };

    let mut data = multipart::server::Multipart::with_body(body.as_slice(), boundary);
    let file = match data.read_entry() {
        Ok(file) => file,
        Err(error) => return response::to_vercel_message(200, error.to_string()),
    };

    let Some(mut file) = file else {
        return response::to_vercel_message(200, "Invalid form data, missing file");
    };
    let mut buffer: Vec<u8> = vec![];

    if let Err(error) = file.data.read_to_end(&mut buffer) {
        return response::to_vercel_message(200, format!("Could not read body data {error}"));
    }

    // ------------------------------------------------------------
    // Format arguments for the generic handler
    // ------------------------------------------------------------

    let Ok(decimals) = decimals.parse::<u16>() else {
        return response::to_vercel_message(
            200,
            "Decimals query parameter is mandatory and should be a valid integer in order to create a valid campaign!",
        );
    };

    response::to_vercel(handler(decimals.into(), &buffer).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::async_test::{setup_env_vars, SERVER};

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

        assert_eq!(response.status, 200);
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_wrong_header() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount_invalid\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_missing_header() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);

        let csv_data =
            b"address\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_missing_column() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_invalid_address() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0xThisIsNotAnAddress,100.0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_duplicated_addresses() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,100.0\n0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_invalid_amount() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);

        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,alphanumeric_amount\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_0() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,0\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_negative() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,-1\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_with_wrong_precision() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491,1.1234\n0xf976aF93B0A5A9F55A7f285a3B5355B8575Eb5bc,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }
}
