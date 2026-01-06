use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Struct that represents the abstraction of an airstream campaign recipient
#[derive(Deserialize, Serialize, Debug, ToSchema)]
pub struct RecipientDto {
    /// Blockchain address of the recipient
    pub address: String,
    /// Amount the recipient will receive
    pub amount: String,
}

/// Struct that represents the abstraction of an airstream campaign
#[derive(Deserialize, Serialize, Debug, ToSchema)]
pub struct PersistentCampaignDto {
    /// Total amount to be distributed in the campaign
    pub total_amount: String,
    /// Number of recipients in the campaign
    pub number_of_recipients: i32,
    /// Merkle root hash of the campaign
    pub root: String,
    /// Serialized merkle tree data
    pub merkle_tree: String,
    /// List of all recipients in the campaign
    pub recipients: Vec<RecipientDto>,
}
