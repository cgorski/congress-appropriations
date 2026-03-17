#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AnthropicError {
    #[error("Rate limited (429): retry after backoff")]
    RateLimited,
    #[error("Overloaded (529): API is overloaded")]
    Overloaded,
    #[error("Authentication error: invalid API key")]
    Authentication,
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Server error ({status}): {message}")]
    ServerError { status: u16, message: String },
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(String),
}
