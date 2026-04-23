use crate::{
    csv_campaign_parser::CampaignCsvParsed,
    data_objects::{
        dto::{PersistentCampaignDto, RecipientDto},
        response::{self, UploadSuccessResponse, ValidationErrorResponse},
    },
    services::ipfs::{try_deserialize_pinata_response, upload_to_ipfs},
    utils::{
        auth, request,
        solana_merkle::{MerkleLeaf, MerkleTree},
    },
};

use csv::ReaderBuilder;
use std::io::Read;

use serde_json::json;
use vercel_runtime as Vercel;

/// Create request common handler. It validates the received data, creates the merkle tree and uploads it to ipfs.
async fn handler(decimals: usize, buffer: &[u8]) -> response::R {
    let rdr = ReaderBuilder::new().from_reader(buffer);
    let parsed_csv = match CampaignCsvParsed::build_solana(rdr, decimals) {
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

    let leaves: Vec<MerkleLeaf> = parsed_csv
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| MerkleLeaf { index: i as u32, recipient: r.address.clone(), amount: r.amount as u64 })
        .collect();

    let tree = MerkleTree::build_tree(leaves);

    let tree_json = tree.dump().unwrap();

    let dto = PersistentCampaignDto {
        total_amount: parsed_csv.total_amount.to_string(),
        number_of_recipients: parsed_csv.number_of_recipients,
        merkle_tree: tree_json,
        root: tree.root_hex(),
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
        root: tree.root_hex(),
        cid: deserialized_response.ipfs_hash,
    });

    response::ok(response_json)
}

/// Vercel specific handler for the create endpoint
pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
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
    let Some(decimals) = query.get("decimals") else {
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
    else {
        return response::to_vercel_message(200, "Invalid content type header");
    };

    let body = req.body().to_vec();

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

        let csv_data = b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,100.0\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 200);
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_wrong_header() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount_invalid\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,100.0\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_missing_header() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);

        let csv_data =
            b"address\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_missing_column() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_invalid_address() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =
            b"address,amount\n0xThisIsNotAnAddress,100.0\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_duplicated_addresses() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data =b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,100.0\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_invalid_amount() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);

        let csv_data = b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,alphanumeric_amount\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_0() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,0\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_negative() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,-1\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }

    #[tokio::test]
    async fn test_csv_with_row_with_amount_with_wrong_precision() {
        let server = SERVER.lock().await;
        setup_env_vars(&server);
        let csv_data = b"address,amount\n9jDBxhUrFx1AFeQzWr8oVEsyMEM2AC3KE4chQr18tV1Y,1.1234\n2wSs9UdwwnLjsjk9bMpErZ81BxaVAqXhtvdGnbNQPs6E,200.0";
        let response = handler(2, csv_data).await;

        assert_eq!(response.status, 400);
        drop(server);
    }
}
