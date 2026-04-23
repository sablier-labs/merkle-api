use crate::{
    data_objects::{
        dto::PersistentCampaignDto,
        query_param::Eligibility,
        response::{self, EligibilityResponse},
    },
    services::ipfs::download_from_ipfs,
    utils::{auth, request},
};
use merkle_tree_rs::standard::{LeafType, StandardMerkleTree, StandardMerkleTreeData};

use serde_json::json;

use vercel_runtime as Vercel;

/// Eligibility request common handler. It downloads data from IPFS and determines if an address is eligible for an
/// airstream campaign.
pub async fn handler(eligibility: Eligibility) -> response::R {
    let Ok(ipfs_data) = download_from_ipfs::<PersistentCampaignDto>(&eligibility.cid).await else {
        return response::message(500, "There was a problem processing your request: Bad CID provided");
    };

    let Some(recipient_index) =
        ipfs_data.recipients.iter().position(|r| r.address.to_lowercase() == eligibility.address.to_lowercase())
    else {
        return response::message(400, "The provided address is not eligible for this campaign");
    };

    let Ok(tree_data) = serde_json::from_str::<StandardMerkleTreeData>(&ipfs_data.merkle_tree) else {
        return response::message(500, "Malformed merkle tree in IPFS data");
    };

    let tree = StandardMerkleTree::load(tree_data);

    let proof = tree.get_proof(LeafType::Number(recipient_index));

    let response_json = json!(&EligibilityResponse {
        index: recipient_index,
        proof,
        address: ipfs_data.recipients[recipient_index].address.clone(),
        amount: ipfs_data.recipients[recipient_index].amount.clone(),
    });
    response::ok_immutable(response_json)
}

/// Vercel specific handler for the create eligibility
pub async fn handler_to_vercel(req: Vercel::Request) -> Result<Vercel::Response<Vercel::ResponseBody>, Vercel::Error> {
    if !auth::is_authorized(&req) {
        return response::to_vercel_message(401, "Bad authentication process provided.");
    }

    // ------------------------------------------------------------
    // Extract query parameters from the URL: address, cid
    // ------------------------------------------------------------

    let query = request::query_params(&req);

    // ------------------------------------------------------------
    // Format arguments for the generic handler
    // ------------------------------------------------------------

    let fallback = String::new();
    let params = Eligibility {
        address: query.get("address").unwrap_or(&fallback).clone(),
        cid: query.get("cid").unwrap_or(&fallback).clone(),
    };

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
            .with_body(r#"{"root": "root", "total_amount": "10", "number_of_recipients": 1, "merkle_tree":"{\"format\":\"standard-v1\",\"tree\":[\"0x23bb7a869a407bc69b27975acff039dfe6a6abe5e3da626e98623d70137eb320\"],\"values\":[{\"value\":[\"0\",\"0x9ad7cad4f10d0c3f875b8a2fd292590490c9f491\",\"5000\"],\"tree_index\":0}],\"leaf_encoding\":[\"uint\",\"address\",\"uint256\"]}", "recipients": [{ "address": "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491", "amount": "10"}]}"#)
            .create();

        let validity = Eligibility {
            cid: "valid_cid".to_string(),
            address: "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491".to_string(),
        };
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

        let validity = Eligibility {
            cid: "invalid_cid".to_string(),
            address: "0x0x9ad7CAD4F10D0c3f875b8a2fd292590490c9f491".to_string(),
        };
        let response = handler(validity).await;
        assert_eq!(response.status, 500);
        mock.assert();
        drop(server);
    }
}
