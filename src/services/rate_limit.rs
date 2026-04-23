use crate::services::redis::RedisClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Reject,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub scope: &'static str,
    pub limit: u64,
    pub window_secs: u64,
}

/// Fixed-window per-IP rate limit. Keyed by `rl:<scope>:<ip>` in Redis.
/// Fail-open: when Redis is unconfigured or unreachable, requests are allowed so
/// the API stays available.
pub async fn check(cfg: Config, ip: &str) -> Decision {
    let Some(redis) = RedisClient::from_env() else {
        return Decision::Allow;
    };

    let key = format!("rl:{}:{}", cfg.scope, ip);
    let count = match redis.incr(&key).await {
        Ok(c) => c,
        Err(_) => return Decision::Allow,
    };

    if count == 1 {
        let _ = redis.expire(&key, cfg.window_secs).await;
    }

    if count as u64 > cfg.limit {
        Decision::Reject
    } else {
        Decision::Allow
    }
}
