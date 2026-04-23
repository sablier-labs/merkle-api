use vercel_runtime as Vercel;

/// Shared bearer-token check. Returns true only when the `Authorization` header
/// is exactly `Bearer <MERKLE_API_BEARER_TOKEN>`. Fail-closed on misconfiguration:
/// missing or empty env var rejects every request.
pub fn is_authorized(req: &Vercel::Request) -> bool {
    let expected = match std::env::var("MERKLE_API_BEARER_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => return false,
    };

    let Some(header) = req.headers().get("Authorization") else {
        return false;
    };
    let Ok(value) = header.to_str() else {
        return false;
    };

    value == format!("Bearer {expected}")
}
