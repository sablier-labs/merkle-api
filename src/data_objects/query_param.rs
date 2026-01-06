use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};

/// Query parameters for eligibility endpoint
#[derive(Deserialize, IntoParams, ToSchema)]
pub struct Eligibility {
    /// Blockchain address to check eligibility for
    #[serde(default = "default_string")]
    pub address: String,

    /// IPFS CID of the campaign
    #[serde(default = "default_string")]
    pub cid: String,
}

fn default_string() -> String {
    "".to_string()
}

/// Query parameters for create endpoint
#[derive(Deserialize, IntoParams, ToSchema)]
pub struct Create {
    /// Number of decimal places for the token amounts
    #[serde(default = "default_string")]
    pub decimals: String,
}

/// Query parameters for validity endpoint
#[derive(Deserialize, IntoParams, ToSchema)]
pub struct Validity {
    /// IPFS CID of the campaign to validate
    #[serde(default = "default_string")]
    pub cid: String,
}
