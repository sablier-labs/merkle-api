use std::time::Duration;

use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

static HTTP: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(1))
        .build()
        .expect("failed to build reqwest client for Upstash")
});

#[derive(Debug)]
pub enum RedisError {
    Request(reqwest::Error),
    Remote(String),
    Parse(serde_json::Error),
}

impl std::fmt::Display for RedisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request(e) => write!(f, "redis request error: {e}"),
            Self::Remote(m) => write!(f, "redis remote error: {m}"),
            Self::Parse(e) => write!(f, "redis parse error: {e}"),
        }
    }
}

impl std::error::Error for RedisError {}

impl From<reqwest::Error> for RedisError {
    fn from(e: reqwest::Error) -> Self {
        Self::Request(e)
    }
}

impl From<serde_json::Error> for RedisError {
    fn from(e: serde_json::Error) -> Self {
        Self::Parse(e)
    }
}

#[derive(Deserialize)]
struct RestResponse {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<String>,
}

pub struct RedisClient {
    url: String,
    token: String,
}

impl RedisClient {
    /// Construct a client from `UPSTASH_REDIS_REST_URL` + `UPSTASH_REDIS_REST_TOKEN`.
    /// Returns `None` when either env var is missing or empty — callers treat this as fail-open.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("UPSTASH_REDIS_REST_URL").ok()?;
        let token = std::env::var("UPSTASH_REDIS_REST_TOKEN").ok()?;
        if url.is_empty() || token.is_empty() {
            return None;
        }
        Some(Self { url, token })
    }

    async fn cmd(&self, args: &[&str]) -> Result<Value, RedisError> {
        let response = HTTP.post(&self.url).bearer_auth(&self.token).json(&args).send().await?;
        let body: RestResponse = response.json().await?;
        if let Some(message) = body.error {
            return Err(RedisError::Remote(message));
        }
        Ok(body.result.unwrap_or(Value::Null))
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, RedisError> {
        match self.cmd(&["GET", key]).await? {
            Value::String(s) => Ok(Some(s)),
            Value::Null => Ok(None),
            other => Err(RedisError::Remote(format!("unexpected GET response: {other}"))),
        }
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<(), RedisError> {
        self.cmd(&["SET", key, value]).await?;
        Ok(())
    }

    pub async fn setex(&self, key: &str, ttl_secs: u64, value: &str) -> Result<(), RedisError> {
        let ttl = ttl_secs.to_string();
        self.cmd(&["SETEX", key, &ttl, value]).await?;
        Ok(())
    }

    pub async fn incr(&self, key: &str) -> Result<i64, RedisError> {
        match self.cmd(&["INCR", key]).await? {
            Value::Number(n) => n.as_i64().ok_or_else(|| RedisError::Remote(format!("non-i64 INCR response: {n}"))),
            other => Err(RedisError::Remote(format!("unexpected INCR response: {other}"))),
        }
    }

    pub async fn expire(&self, key: &str, ttl_secs: u64) -> Result<(), RedisError> {
        let ttl = ttl_secs.to_string();
        self.cmd(&["EXPIRE", key, &ttl]).await?;
        Ok(())
    }
}
