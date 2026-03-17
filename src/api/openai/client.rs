use super::types::{EmbeddingRequest, EmbeddingResponse};
use anyhow::{Context, Result};
use reqwest::Client;

pub struct OpenAIClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl OpenAIClient {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;
        Ok(Self {
            http: Client::new(),
            api_key,
            base_url: "https://api.openai.com/v1".to_string(),
        })
    }

    pub async fn embed(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let url = format!("{}/embeddings", self.base_url);
        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error {status}: {body}");
        }

        response
            .json()
            .await
            .context("Failed to parse embedding response")
    }
}
