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

/// Extract the original client IP from Vercel's forwarding headers. Falls back
/// to `"unknown"` so the rate-limit key is always populated (pooled under a
/// single bucket, which is the safer default than silently skipping the limit).
pub fn client_ip(req: &Vercel::Request) -> String {
    let headers = req.headers();

    if let Some(value) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = value.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    if let Some(value) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    "unknown".to_string()
}
