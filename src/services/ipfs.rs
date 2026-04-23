use dotenvy::dotenv;
use reqwest::multipart::{Form, Part};

use serde_json::json;

use crate::data_objects::dto::PersistentCampaignDto;
use serde::{de::DeserializeOwned, Deserialize};

/// The success response after an upload request to Pinata
#[derive(Deserialize, Debug)]
pub struct PinataSuccess {
    #[serde(rename = "IpfsHash")]
    pub ipfs_hash: String,
}

/// Errors surfaced from `download_from_ipfs`. Callers today only check `is_err()`,
/// but the variants are kept distinct so callers can distinguish permanent (client)
/// from transient (upstream) failures when they want to.
#[derive(Debug)]
pub enum IpfsError {
    Request(reqwest::Error),
    Deserialize(serde_json::Error),
    InvalidCid,
    NotFound,
    Upstream { status: u16, body: String },
}

impl std::fmt::Display for IpfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(e) => write!(f, "ipfs request error: {e}"),
            Self::Deserialize(e) => write!(f, "ipfs deserialize error: {e}"),
            Self::InvalidCid => write!(f, "invalid cid format"),
            Self::NotFound => write!(f, "cid not found"),
            Self::Upstream { status, body } => {
                write!(f, "ipfs upstream error {status}: {body}")
            }
        }
    }
}

impl std::error::Error for IpfsError {}

impl From<reqwest::Error> for IpfsError {
    fn from(e: reqwest::Error) -> Self {
        Self::Request(e)
    }
}

impl From<serde_json::Error> for IpfsError {
    fn from(e: serde_json::Error) -> Self {
        Self::Deserialize(e)
    }
}

/// Deserialize the text response returned by Pinata API into PinataSuccess
///
/// # Examples
///
/// ```
/// use serde;
/// use sablier_merkle_api::services::ipfs::{try_deserialize_pinata_response, PinataSuccess};
///
/// let result_ok: Result<PinataSuccess, serde_json::Error> = try_deserialize_pinata_response(r#"{"IpfsHash": "test_hash", "PinSize": 123, "Timestamp": "2023-04-05T00:00:00Z"}"#);
/// let result_error: Result<PinataSuccess, serde_json::Error> = try_deserialize_pinata_response("Error message");
/// assert!(result_ok.is_ok());
/// assert!(result_error.is_err());
/// ```
pub fn try_deserialize_pinata_response(response_body: &str) -> Result<PinataSuccess, serde_json::Error> {
    serde_json::from_str::<PinataSuccess>(response_body)
}

/// Upload and pin a JSON representing a valid processed airstream campaign
pub async fn upload_to_ipfs(data: &PersistentCampaignDto) -> Result<String, reqwest::Error> {
    dotenv().ok();
    let pinata_api_key = std::env::var("PINATA_API_KEY").expect("PINATA_API_KEY must be set");
    let pinata_secret_api_key = std::env::var("PINATA_SECRET_API_KEY").expect("PINATA_SECRET_API_KEY must be set");
    let pinata_api_server = std::env::var("PINATA_API_SERVER").expect("PINATA_API_SERVER must be set");

    let client = reqwest::Client::new();

    let api_endpoint = format!("{pinata_api_server}/pinning/pinFileToIPFS");

    let serialized_data = json!(data);
    let bytes = serde_json::to_vec(&serialized_data).unwrap();
    let part = Part::bytes(bytes).file_name("data.json").mime_str("application/json")?;

    let form = Form::new().part("file", part);

    let response = client
        .post(api_endpoint)
        .header("pinata_api_key", pinata_api_key)
        .header("pinata_secret_api_key", pinata_secret_api_key)
        .multipart(form)
        .send()
        .await?;

    let text_response = response.text().await?;
    Ok(text_response)
}

/// Conservative CID sanity check. Keeps genuine CIDs (base58/base32 strings) intact
/// while rejecting inputs that could inject `?`, `#`, `/`, or whitespace into the
/// gateway URL we build via `format!`. `_` and `-` are allowed so test fixtures and
/// future base64url-style CIDs (RFC 4648) pass through.
fn is_cid_format_valid(cid: &str) -> bool {
    !cid.is_empty() && cid.len() <= 120 && cid.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Download the content from a specified CID through Pinata. Callers rely on
/// Vercel's edge cache (via `Cache-Control` on the outer response) to avoid
/// re-fetching the same CID.
pub async fn download_from_ipfs<T: DeserializeOwned>(cid: &str) -> Result<T, IpfsError> {
    if !is_cid_format_valid(cid) {
        return Err(IpfsError::InvalidCid);
    }

    let raw = fetch_raw_from_pinata(cid).await?;
    serde_json::from_str(&raw).map_err(IpfsError::from)
}

async fn fetch_raw_from_pinata(cid: &str) -> Result<String, IpfsError> {
    dotenv().ok();
    let ipfs_gateway = std::env::var("IPFS_GATEWAY").expect("IPFS_GATEWAY must be set");
    let pinata_access_token = std::env::var("PINATA_ACCESS_TOKEN").expect("PINATA_ACCESS_TOKEN must be set");
    let ipfs_url = format!("{ipfs_gateway}/{cid}?pinataGatewayToken={pinata_access_token}");

    let response = reqwest::get(&ipfs_url).await?;
    let status = response.status();
    let text = response.text().await?;

    if status.is_success() {
        Ok(text)
    } else if status.is_client_error() {
        Err(IpfsError::NotFound)
    } else {
        Err(IpfsError::Upstream { status: status.as_u16(), body: text })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::async_test::{setup_env_vars, SERVER};

    #[test]
    fn try_deserialize_pinata_response_success() {
        let result: Result<PinataSuccess, serde_json::Error> = try_deserialize_pinata_response(
            r#"{"IpfsHash": "test_hash", "PinSize": 123, "Timestamp": "2023-04-05T00:00:00Z"}"#,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn try_deserialize_pinata_response_fail() {
        let result: Result<PinataSuccess, serde_json::Error> = try_deserialize_pinata_response("Error message");
        assert!(result.is_err());
    }

    #[test]
    fn cid_validation_accepts_cids() {
        assert!(is_cid_format_valid("validcid"));
        assert!(is_cid_format_valid("valid_cid"));
        assert!(is_cid_format_valid("valid-cid"));
        assert!(is_cid_format_valid("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"));
        assert!(is_cid_format_valid("QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"));
    }

    #[test]
    fn cid_validation_rejects_bad_inputs() {
        assert!(!is_cid_format_valid(""));
        assert!(!is_cid_format_valid("has space"));
        assert!(!is_cid_format_valid("has?query"));
        assert!(!is_cid_format_valid("has/slash"));
        assert!(!is_cid_format_valid("has#fragment"));
        assert!(!is_cid_format_valid(&"a".repeat(121)));
    }

    #[tokio::test]
    async fn test_upload_to_ipfs_ok() {
        let mut server = SERVER.lock().await;
        setup_env_vars(&server);
        // Set up mock server
        let mock = server
            .mock("POST", "/pinning/pinFileToIPFS")
            .with_status(200)
            .with_body(r#"{"IpfsHash": "test_hash", "PinSize": 123, "Timestamp": "2021-01-01T00:00:00Z"}"#)
            .create();

        // Call the function with a test data object
        let data = PersistentCampaignDto {
            total_amount: "128".to_string(),
            number_of_recipients: 4,
            root: "test_root".to_string(),
            merkle_tree: "test_merkle".to_string(),
            recipients: Vec::new(),
        };
        let result = upload_to_ipfs(&data).await;

        assert!(result.is_ok());
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn test_upload_to_ipfs_error() {
        let mut server = SERVER.lock().await;

        setup_env_vars(&server);
        // Set up mock server
        let mock = server
            .mock("POST", "/pinning/pinFileToIPFS")
            .with_status(500)
            .with_body(r#"{"code": "500", "message": "Internal server error"}"#)
            .create();

        // Call the function with a test data object
        let data = PersistentCampaignDto {
            total_amount: "128".to_string(),
            number_of_recipients: 4,
            root: "test_root".to_string(),
            merkle_tree: "test_merkle".to_string(),
            recipients: Vec::new(),
        };
        let result = upload_to_ipfs(&data).await;

        let result = result.unwrap();
        let deserialized_response = try_deserialize_pinata_response(&result);
        assert!(deserialized_response.is_err());
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn test_download_from_ipfs_success() {
        let mut server = SERVER.lock().await;

        setup_env_vars(&server);

        // Set up mock server
        let mock = server
            .mock("GET", "/valid_cid?pinataGatewayToken=mock_pinata_access_token")
            .with_status(200)
            .with_body(r#"{"IpfsHash": "test_hash", "PinSize": 123, "Timestamp": "2021-01-01T00:00:00Z"}"#)
            .create();

        let result: Result<PinataSuccess, _> = download_from_ipfs("valid_cid").await;
        assert!(result.is_ok());
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn test_download_from_ipfs_error() {
        let mut server = SERVER.lock().await;

        setup_env_vars(&server);

        // Set up mock server
        let mock = server
            .mock("GET", "/valid_cid?pinataGatewayToken=mock_pinata_access_token")
            .with_status(500)
            .with_body(r#"{"code": "500", "message": "Internal server error"}"#)
            .create();

        let result: Result<PinataSuccess, _> = download_from_ipfs("valid_cid").await;
        assert!(result.is_err());
        mock.assert();
        drop(server);
    }
}
