use crate::{
    data_objects::{query_param::Eligibility, response},
    services::ipfs::IpfsError,
    utils::logging,
};
use serde_json::json;
use std::time::Duration;

pub(super) const SLOW_ELIGIBILITY_THRESHOLD: Duration = Duration::from_secs(2);

pub(super) fn ipfs_error_response(error: &IpfsError) -> response::R {
    response::message(error.response_status(), ipfs_error_message(error))
}

pub(super) fn log_ipfs_error(endpoint: &str, eligibility: &Eligibility, error: &IpfsError, elapsed: Duration) {
    let elapsed_ms = elapsed.as_millis() as u64;

    logging::log_event(
        "error",
        "eligibility_ipfs_error",
        json!({
            "endpoint": endpoint,
            "cid": eligibility.cid.as_str(),
            "address_hash": logging::hash_identifier(&eligibility.address),
            "error_kind": error.kind(),
            "provider_status": error.provider_status(),
            "elapsed_ms": elapsed_ms,
            "response_status": error.response_status(),
        }),
    );
}

pub(super) fn log_malformed_tree(
    endpoint: &str,
    eligibility: &Eligibility,
    elapsed: Duration,
    bytes: usize,
    recipient_count: usize,
) {
    let elapsed_ms = elapsed.as_millis() as u64;

    logging::log_event(
        "error",
        "eligibility_malformed_tree",
        json!({
            "endpoint": endpoint,
            "cid": eligibility.cid.as_str(),
            "address_hash": logging::hash_identifier(&eligibility.address),
            "elapsed_ms": elapsed_ms,
            "bytes": bytes,
            "recipient_count": recipient_count,
            "response_status": 500,
        }),
    );
}

pub(super) fn log_failed_proof(endpoint: &str, eligibility: &Eligibility, elapsed: Duration, recipient_index: usize) {
    let elapsed_ms = elapsed.as_millis() as u64;

    logging::log_event(
        "error",
        "eligibility_failed_proof",
        json!({
            "endpoint": endpoint,
            "cid": eligibility.cid.as_str(),
            "address_hash": logging::hash_identifier(&eligibility.address),
            "elapsed_ms": elapsed_ms,
            "recipient_index": recipient_index,
            "response_status": 500,
        }),
    );
}

pub(super) fn log_slow_success(
    endpoint: &str,
    eligibility: &Eligibility,
    elapsed: Duration,
    bytes: usize,
    recipient_count: usize,
) {
    if elapsed < SLOW_ELIGIBILITY_THRESHOLD {
        return;
    }

    let elapsed_ms = elapsed.as_millis() as u64;

    logging::log_event(
        "warn",
        "eligibility_slow",
        json!({
            "endpoint": endpoint,
            "cid": eligibility.cid.as_str(),
            "address_hash": logging::hash_identifier(&eligibility.address),
            "elapsed_ms": elapsed_ms,
            "bytes": bytes,
            "recipient_count": recipient_count,
            "response_status": 200,
        }),
    );
}

fn ipfs_error_message(error: &IpfsError) -> &'static str {
    match error {
        IpfsError::Request(request_error) if request_error.is_timeout() => {
            "There was a problem processing your request: IPFS gateway timeout"
        }
        IpfsError::Request(_) | IpfsError::Upstream { .. } => {
            "There was a problem processing your request: IPFS provider unavailable"
        }
        IpfsError::Deserialize(_) | IpfsError::InvalidCid | IpfsError::NotFound => {
            "There was a problem processing your request: Bad CID provided"
        }
    }
}
