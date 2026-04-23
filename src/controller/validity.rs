use crate::{
    data_objects::{
        dto::PersistentCampaignDto,
        query_param::Validity,
        response::{self, ValidResponse},
    },
    services::ipfs::download_from_ipfs,
    utils::{auth, request},
};

use serde_json::json;

use vercel_runtime as Vercel;

/// Validity request common handler. It downloads data from IPFS and checks if it can be properly deserialized into a
/// `PersistentCampaignDto` struct.
pub async fn handler(validity: Validity) -> response::R {
    let Ok(ipfs_data) = download_from_ipfs::<PersistentCampaignDto>(&validity.cid).await else {
        return response::message(500, "Bad CID or invalid file format provided.");
    };

    let response_json = json!(&ValidResponse {
        root: ipfs_data.root,
        total: ipfs_data.total_amount,
        recipients: ipfs_data.number_of_recipients.to_string(),
        cid: validity.cid
    });
    response::ok_immutable(response_json)
}

/// Vercel specific handler for the validity endpoint
pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    if !auth::is_authorized(&req) {
        return response::to_vercel_message(401, "Bad authentication process provided.");
    }

    // ------------------------------------------------------------
    // Extract query parameters from the URL: cid
    // ------------------------------------------------------------

    let query = request::query_params(&req);

    // ------------------------------------------------------------
    // Format arguments for the generic handler
    // ------------------------------------------------------------

    let fallback = String::new();
    let params = Validity { cid: query.get("cid").unwrap_or(&fallback).clone() };

    response::to_vercel(handler(params).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::async_test::{setup_env_vars, SERVER};

    #[tokio::test]
    async fn handler_success_response() {
        let mut server = SERVER.lock().await;

        setup_env_vars(&server);

        let mock = server
            .mock("GET", "/valid_cid?pinataGatewayToken=mock_pinata_access_token")
            .with_status(200)
            .with_body(r#"{"root": "root", "total_amount": "123", "number_of_recipients": 3, "merkle_tree":"asd", "recipients": []}"#)
            .create();

        let validity = Validity { cid: "valid_cid".to_string() };
        let response = handler(validity).await;
        assert_eq!(response.status, 200);
        mock.assert();
        drop(server);
    }

    #[tokio::test]
    async fn handler_error_response() {
        let mut server = SERVER.lock().await;

        setup_env_vars(&server);

        let mock = server
            .mock("GET", "/invalid_cid?pinataGatewayToken=mock_pinata_access_token")
            .with_status(500)
            .with_body(r#"{"message": "Bad request"}"#)
            .create();

        let validity = Validity { cid: "invalid_cid".to_string() };
        let response = handler(validity).await;
        assert_eq!(response.status, 500);
        mock.assert();
        drop(server);
    }
}
