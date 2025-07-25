use crate::{
    data_objects::{
        dto::PersistentCampaignDto,
        query_param::Eligibility,
        response::{self, EligibilityResponse, GeneralErrorResponse},
    },
    services::ipfs::download_from_ipfs,
    utils::solana_merkle::MerkleTree,
    WebResult,
};

use serde_json::json;
use std::{collections::HashMap, str};
use url::Url;

use vercel_runtime as Vercel;
use warp::Filter;

/// Bearer token guard
fn is_authorized(req: &Vercel::Request) -> bool {
    let headers = req.headers();
    let expected_token = std::env::var("MERKLE_API_BEARER_TOKEN").expect("MERKLE_API_BEARER_TOKEN must be set");

    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            return auth_str == format!("Bearer {expected_token}");
        }
    }

    false
}

/// Eligibility request common handler. It downloads data from IPFS and determines if an address is eligible for an
/// airstream campaign.
pub async fn handler(eligibility: Eligibility) -> response::R {
    let ipfs_data = download_from_ipfs::<PersistentCampaignDto>(&eligibility.cid).await;
    if ipfs_data.is_err() {
        let response_json = json!(GeneralErrorResponse {
            message: "There was a problem processing your request: Bad CID provided".to_string(),
        });

        return response::internal_server_error(response_json);
    }
    let ipfs_data = ipfs_data.unwrap();
    let recipient_index =
        ipfs_data.recipients.iter().position(|r| r.address.to_lowercase() == eligibility.address.to_lowercase());

    if recipient_index.is_none() {
        let response_json = json!(GeneralErrorResponse {
            message: String::from("The provided address is not eligible for this campaign"),
        });

        return response::bad_request(response_json);
    }

    let recipient_index = recipient_index.unwrap();

    let tree = MerkleTree::load(&ipfs_data.merkle_tree).unwrap();

    let proof = tree.get_proof(recipient_index as u32).unwrap();

    let response_json = json!(&EligibilityResponse {
        index: recipient_index,
        proof,
        address: ipfs_data.recipients[recipient_index].address.clone(),
        amount: ipfs_data.recipients[recipient_index].amount.clone(),
    });
    response::ok(response_json)
}

/// Warp specific handler for the eligibility endpoint
pub async fn handler_to_warp(eligibility: Eligibility) -> WebResult<impl warp::Reply> {
    let result = handler(eligibility).await;
    Ok(response::to_warp(result))
}

/// Vercel specific handler for the create eligibility
pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    // ------------------------------------------------------------
    // Extract query parameters from the URL: address, cid
    // ------------------------------------------------------------

    let url = Url::parse(&req.uri().to_string()).unwrap();
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();

    if is_authorized(&req) {
        // ------------------------------------------------------------
        //Format arguments for the generic handler
        // ------------------------------------------------------------

        let fallback = String::from("");
        let params = Eligibility {
            address: query.get("address").unwrap_or(&fallback).clone(),
            cid: query.get("cid").unwrap_or(&fallback).clone(),
        };

        let result = handler(params).await;

        response::to_vercel(result)
    } else {
        let response_json =
            json!(GeneralErrorResponse { message: String::from("Bad authentication process provided.") });

        response::to_vercel(response::unauthorized(response_json))
    }
}

/// Bind the route with the handler for the Warp handler.
pub fn build_route() -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "eligibility_solana")
        .and(warp::get())
        .and(warp::query::query::<Eligibility>())
        .and_then(handler_to_warp)
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
            .with_body(r#"{"root": "root", "total_amount": "10", "number_of_recipients": 1, "merkle_tree": "{\"root\":\"e51044cc70a2ed388c9fed090e7f1401278de3f5fec8a0d0c6b5176c9ebe3b93\",\"tree\":[[\"410c2c7cb39bf8cc15b1e22fc5b9c26be08465174ccef0be090d9d9df86d03ad\",\"1f605d6b20676921f61532c385082aae4619ba91dfb83c71bf1bc43678626119\",\"158db4f6ff3d0547cef89e6125c2c39052d1b2b288a8db9742958cc1b80fcb43\",\"77a70b41a193dc0a1e9a07dca4a3f2fb40c37282a6d18849dd7af36b684590ca\"],[\"a6a693f5474548569bfd931d4af466a50a5eb0374d895071f647393ff6da241b\",\"35b4f28cb601668e6f89cb6eace3a2d845e700f718182bdf237f17811ead81a6\"],[\"e51044cc70a2ed388c9fed090e7f1401278de3f5fec8a0d0c6b5176c9ebe3b93\"]]}", "recipients": [{ "address": "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491", "amount": "10"}]}"#)
            .create();

        let validity = Eligibility {
            cid: "valid_cid".to_string(),
            address: "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491".to_string(),
        };
        let response = handler(validity).await;
        assert_eq!(response.status, warp::http::StatusCode::OK.as_u16());
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

        let validity = Eligibility {
            cid: "invalid_cid".to_string(),
            address: "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491".to_string(),
        };
        let response = handler(validity).await;
        assert_eq!(response.status, warp::http::StatusCode::INTERNAL_SERVER_ERROR.as_u16());
        mock.assert();
        drop(server);
    }
}
