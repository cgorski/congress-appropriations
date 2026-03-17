/// Errors returned by the Congress.gov API client.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Rate limited (429)")]
    RateLimited,

    #[error("Not found (404): {0}")]
    NotFound(String),

    #[error("Server error ({0})")]
    ServerError(u16),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}
