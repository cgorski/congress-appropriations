use super::error::ApiError;
use serde::de::DeserializeOwned;
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};

/// Client for the Congress.gov API (v3).
#[derive(Debug, Clone)]
pub struct CongressClient {
    pub(crate) http: reqwest::Client,
    pub(crate) api_key: String,
    pub(crate) base_url: String,
    pub(crate) delay: Duration,
}

/// Builder for constructing a [`CongressClient`] with custom settings.
#[derive(Debug, Clone)]
pub struct CongressClientBuilder {
    api_key: String,
    base_url: String,
    delay: Duration,
}

impl CongressClientBuilder {
    fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.congress.gov/v3".to_string(),
            delay: Duration::from_millis(750),
        }
    }

    /// Override the base URL (useful for testing).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Override the per-request delay (default 750ms).
    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Build the [`CongressClient`].
    pub fn build(self) -> CongressClient {
        let http = reqwest::Client::builder()
            .user_agent("congress-api-rs/0.1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("failed to build reqwest client");

        CongressClient {
            http,
            api_key: self.api_key,
            base_url: self.base_url,
            delay: self.delay,
        }
    }
}

impl CongressClient {
    /// Create a new client with the given API key and default settings.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self::builder_with_key(api_key).build()
    }

    /// Create a client by reading `CONGRESS_API_KEY` from the environment.
    pub fn from_env() -> Result<Self, ApiError> {
        let api_key = std::env::var("CONGRESS_API_KEY").map_err(|_| {
            ApiError::InvalidInput("CONGRESS_API_KEY environment variable not set".to_string())
        })?;
        Ok(Self::new(api_key))
    }

    /// Return a builder pre-configured with a placeholder API key.
    /// Use [`CongressClient::builder_with_key`] to supply the key up front.
    pub fn builder() -> CongressClientBuilder {
        let api_key = std::env::var("CONGRESS_API_KEY").unwrap_or_default();
        CongressClientBuilder::new(api_key)
    }

    /// Return a builder with the given API key.
    pub fn builder_with_key(api_key: impl Into<String>) -> CongressClientBuilder {
        CongressClientBuilder::new(api_key.into())
    }

    /// Perform a GET request against the Congress.gov API.
    ///
    /// Automatically appends `api_key` and `format=json` query parameters.
    /// Retries up to 4 times with exponential back-off on 429 / 503 / 5xx responses.
    /// Sleeps [`self.delay`] between calls to stay under rate limits.
    pub async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, ApiError> {
        let url = format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        );

        let max_retries: u32 = 4;
        let mut attempt: u32 = 0;

        let param_str: String = params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        let display_path = if param_str.is_empty() {
            path.to_string()
        } else {
            format!("{path}?{param_str}")
        };

        loop {
            attempt += 1;
            let start = Instant::now();

            debug!(path = %display_path, attempt, "→ GET");

            let mut request = self
                .http
                .get(&url)
                .query(&[("api_key", self.api_key.as_str()), ("format", "json")]);

            for (key, value) in params {
                request = request.query(&[(key, value)]);
            }

            let response = request.send().await?;
            let status = response.status();
            let elapsed = start.elapsed();

            if status.is_success() {
                let bytes = response.bytes().await?;
                let size = bytes.len();
                trace!(
                    path = %display_path,
                    status = %status,
                    bytes = size,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "← OK"
                );

                // Respect delay between successful calls
                tokio::time::sleep(self.delay).await;

                let parsed: T = serde_json::from_slice(&bytes).inspect_err(|e| {
                    debug!(
                        path = %display_path,
                        error = %e,
                        body_preview = %String::from_utf8_lossy(&bytes[..bytes.len().min(300)]),
                        "deserialization failed"
                    );
                })?;
                return Ok(parsed);
            }

            let code = status.as_u16();
            info!(
                path = %display_path,
                status = code,
                elapsed_ms = elapsed.as_millis() as u64,
                attempt,
                "← HTTP {code}"
            );

            match code {
                404 => {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "unknown".to_string());
                    return Err(ApiError::NotFound(format!("{display_path}: {body}")));
                }
                429 => {
                    if attempt > max_retries {
                        return Err(ApiError::RateLimited);
                    }
                    let backoff = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
                    warn!(
                        attempt,
                        backoff_ms = backoff.as_millis() as u64,
                        "⏳ rate limited (429), backing off"
                    );
                    tokio::time::sleep(backoff).await;
                }
                503 => {
                    if attempt > max_retries {
                        return Err(ApiError::ServerError(503));
                    }
                    let backoff = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
                    warn!(
                        attempt,
                        backoff_ms = backoff.as_millis() as u64,
                        "⏳ service unavailable (503), backing off"
                    );
                    tokio::time::sleep(backoff).await;
                }
                c if (500..600).contains(&c) => {
                    if attempt > max_retries {
                        return Err(ApiError::ServerError(c));
                    }
                    let backoff = Duration::from_millis(1000 * 2u64.pow(attempt - 1));
                    warn!(
                        attempt,
                        status = c,
                        backoff_ms = backoff.as_millis() as u64,
                        "⏳ server error, backing off"
                    );
                    tokio::time::sleep(backoff).await;
                }
                c => {
                    return Err(ApiError::ServerError(c));
                }
            }
        }
    }

    /// Quick connectivity check — fetches H.R. 1 from the 118th Congress.
    pub async fn test_connection(&self) -> Result<(), ApiError> {
        let _: serde_json::Value = self.get("/bill/118/hr/1", &[]).await?;
        Ok(())
    }
}
