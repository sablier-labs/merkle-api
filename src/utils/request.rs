use std::collections::HashMap;
use url::form_urlencoded;
use vercel_runtime as Vercel;

/// Decode query-string parameters from the request URI. Returns an empty map
/// when no query string is present.
pub fn query_params(req: &Vercel::Request) -> HashMap<String, String> {
    let query = req.uri().query().unwrap_or_default();
    form_urlencoded::parse(query.as_bytes()).into_owned().collect()
}
