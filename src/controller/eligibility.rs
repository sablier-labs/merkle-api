use crate::{
    data_objects::{
        dto::PersistentCampaignDto,
        query_param::Eligibility,
        response::{self, EligibilityResponse, GeneralErrorResponse},
    },
    services::ipfs::download_from_ipfs,
    WebResult,
};
use merkle_tree_rs::standard::{LeafType, StandardMerkleTree, StandardMerkleTreeData};

use serde_json::json;
use std::{collections::HashMap, str};
use url::Url;

use vercel_runtime as Vercel;
use warp::Filter;

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

    let tree_data: StandardMerkleTreeData = serde_json::from_str(&ipfs_data.merkle_tree).unwrap();

    let tree = StandardMerkleTree::load(tree_data);

    let proof = tree.get_proof(LeafType::Number(recipient_index));

    let response_json = json!(&EligibilityResponse {
        index: recipient_index,
        proof,
        address: ipfs_data.recipients[recipient_index].address.clone(),
        amount: ipfs_data.recipients[recipient_index].amount.clone(),
    });
    response::ok(response_json)
}

pub async fn handler_to_warp(eligibility: Eligibility) -> WebResult<impl warp::Reply> {
    let result = handler(eligibility).await;
    Ok(response::to_warp(result))
}

pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::Body>, Vercel::Error> {
    // ------------------------------------------------------------
    // Extract query parameters from the URL: address, cid
    // ------------------------------------------------------------

    let url = Url::parse(&req.uri().to_string()).unwrap();
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();

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
}

pub fn build_route() -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "eligibility")
        .and(warp::get())
        .and(warp::query::query::<Eligibility>())
        .and_then(handler_to_warp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn handler_success_response() {
        let mut server = mockito::Server::new_with_port(8000);

        let host = server.host_with_port();
        let parts: Vec<&str> = host.split(':').collect();
        let port = parts[1];
        let server_host = format!("http://localhost:{}/", port);

        std::env::set_var("PINATA_ACCESS_TOKEN", "mock_pinata_access_token");
        std::env::set_var("IPFS_GATEWAY", server_host);

        let mock = server
            .mock("GET", "/valid_cid?pinataGatewayToken=mock_pinata_access_token")
            .with_status(200)
            .with_body(r#"{"root": "root", "total_amount": "10", "number_of_recipients": 1, "merkle_tree":"{\"format\":\"standard-v1\",\"tree\":[\"0x23bb7a869a407bc69b27975acff039dfe6a6abe5e3da626e98623d70137eb320\"],\"values\":[{\"value\":[\"0\",\"0x9ad7cad4f10d0c3f875b8a2fd292590490c9f491\",\"5000\"],\"tree_index\":0}],\"leaf_encoding\":[\"uint\",\"address\",\"uint256\"]}", "recipients": [{ "address": "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491", "amount": "10"}]}"#)
            .create();

        let validity = Eligibility {
            cid: "valid_cid".to_string(),
            address: "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491".to_string(),
        };
        let response = handler(validity).await;
        assert_eq!(response.status, warp::http::StatusCode::OK.as_u16());
        mock.assert();
    }

    #[tokio::test]
    async fn handler_error_response() {
        let mut server = mockito::Server::new_with_port(8000);

        let host = server.host_with_port();
        let parts: Vec<&str> = host.split(':').collect();
        let port = parts[1];
        let server_host = format!("http://localhost:{}/", port);

        std::env::set_var("PINATA_ACCESS_TOKEN", "mock_pinata_access_token");
        std::env::set_var("IPFS_GATEWAY", server_host);

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
    }
}
