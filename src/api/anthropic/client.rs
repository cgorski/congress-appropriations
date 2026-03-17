use super::{
    AnthropicError, ContentBlock, CreateMessageRequest, Message, Role, StopReason, StreamEvent,
    Usage,
};
use futures::StreamExt;
use reqwest::header::CONTENT_TYPE;
use std::time::Duration;
use tracing::{debug, warn};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_RETRIES: u32 = 4;
const BACKOFF_BASE: f64 = 2.0;

pub struct AnthropicClient {
    http: reqwest::Client,
    api_key: String,
    pub default_model: String,
}

impl AnthropicClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
            default_model: "claude-opus-4-6".into(),
        }
    }

    pub fn from_env() -> Result<Self, AnthropicError> {
        let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| AnthropicError::Authentication)?;
        Ok(Self::new(key))
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    pub async fn send_message(
        &self,
        request: &CreateMessageRequest,
    ) -> Result<Message, AnthropicError> {
        let mut last_err = None;

        for attempt in 0..MAX_RETRIES {
            let result = self
                .http
                .post(API_URL)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header(CONTENT_TYPE, "application/json")
                .json(request)
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        let body = response.text().await.map_err(AnthropicError::Network)?;
                        debug!("Anthropic response body length: {}", body.len());
                        let message: Message = serde_json::from_str(&body).map_err(|e| {
                            AnthropicError::Deserialize(format!(
                                "{e}: {}",
                                &body[..body.len().min(500)]
                            ))
                        })?;
                        return Ok(message);
                    }

                    let status_code = status.as_u16();
                    let body = response.text().await.unwrap_or_default();

                    match status_code {
                        401 => return Err(AnthropicError::Authentication),
                        404 => return Err(AnthropicError::NotFound(body)),
                        429 => {
                            let wait =
                                Duration::from_secs_f64(BACKOFF_BASE.powi(attempt as i32 + 1));
                            warn!("Rate limited (429), backing off {wait:?}");
                            tokio::time::sleep(wait).await;
                            last_err = Some(AnthropicError::RateLimited);
                            continue;
                        }
                        529 => {
                            let wait =
                                Duration::from_secs_f64(BACKOFF_BASE.powi(attempt as i32 + 1));
                            warn!("Overloaded (529), backing off {wait:?}");
                            tokio::time::sleep(wait).await;
                            last_err = Some(AnthropicError::Overloaded);
                            continue;
                        }
                        400..=499 => return Err(AnthropicError::InvalidRequest(body)),
                        500..=599 => {
                            let wait = Duration::from_secs_f64(BACKOFF_BASE.powi(attempt as i32));
                            warn!("Server error ({status_code}), retry in {wait:?}");
                            tokio::time::sleep(wait).await;
                            last_err = Some(AnthropicError::ServerError {
                                status: status_code,
                                message: body,
                            });
                            continue;
                        }
                        _ => {
                            return Err(AnthropicError::ServerError {
                                status: status_code,
                                message: body,
                            });
                        }
                    }
                }
                Err(e) => {
                    let wait = Duration::from_secs_f64(BACKOFF_BASE.powi(attempt as i32));
                    warn!("Network error: {e}, retry in {wait:?}");
                    tokio::time::sleep(wait).await;
                    last_err = Some(AnthropicError::Network(e));
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or(AnthropicError::ServerError {
            status: 0,
            message: "Max retries exceeded".into(),
        }))
    }

    /// Quick test: send a simple message and check we get a response
    pub async fn test_connection(&self) -> Result<Message, AnthropicError> {
        use super::builder::MessageBuilder;
        let req = MessageBuilder::new(&self.default_model)
            .user("Respond with exactly: OK")
            .max_tokens(64)
            .build();
        self.send_message(&req).await
    }

    /// Send a message with SSE streaming. Calls `on_event` for each streaming event.
    /// Returns the fully assembled Message when done.
    pub async fn send_message_streaming<F>(
        &self,
        request: &CreateMessageRequest,
        mut on_event: F,
    ) -> Result<Message, AnthropicError>
    where
        F: FnMut(&StreamEvent),
    {
        let mut req = request.clone();
        req.stream = Some(true);

        let response = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header(CONTENT_TYPE, "application/json")
            .json(&req)
            .send()
            .await
            .map_err(AnthropicError::Network)?;

        let status = response.status();
        if !status.is_success() {
            let status_code = status.as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(match status_code {
                401 => AnthropicError::Authentication,
                404 => AnthropicError::NotFound(body),
                429 => AnthropicError::RateLimited,
                529 => AnthropicError::Overloaded,
                400..=499 => AnthropicError::InvalidRequest(body),
                _ => AnthropicError::ServerError {
                    status: status_code,
                    message: body,
                },
            });
        }

        // State for accumulating the message
        let mut message_id = String::new();
        let mut model_name = String::new();
        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut current_thinking = String::new();
        let mut current_text = String::new();
        let mut current_block_type = String::new();
        let mut stop_reason: Option<StopReason> = None;
        let mut usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut done = false;

        while let Some(chunk) = stream.next().await {
            if done {
                break;
            }
            let chunk = chunk.map_err(AnthropicError::Network)?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE frames (separated by \n\n)
            while let Some(boundary) = buffer.find("\n\n") {
                let frame = buffer[..boundary].to_string();
                buffer = buffer[boundary + 2..].to_string();

                // Parse frame lines
                let mut event_type = String::new();
                let mut data_str = String::new();
                for line in frame.lines() {
                    if let Some(rest) = line.strip_prefix("event: ") {
                        event_type = rest.trim().to_string();
                    } else if let Some(rest) = line.strip_prefix("data: ") {
                        data_str = rest.to_string();
                    }
                }

                if data_str.is_empty() {
                    continue;
                }

                let data: serde_json::Value = match serde_json::from_str(&data_str) {
                    Ok(v) => v,
                    Err(_) => continue,
                };

                let evt = data["type"].as_str().unwrap_or(&event_type);

                match evt {
                    "message_start" => {
                        if let Some(msg) = data.get("message") {
                            message_id = msg["id"].as_str().unwrap_or_default().to_string();
                            model_name = msg["model"].as_str().unwrap_or_default().to_string();
                            if let Some(u) = msg.get("usage") {
                                usage.input_tokens = u["input_tokens"].as_u64().unwrap_or(0) as u32;
                                usage.cache_creation_input_tokens =
                                    u["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32;
                                usage.cache_read_input_tokens =
                                    u["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32;
                            }
                        }
                    }
                    "content_block_start" => {
                        let index = data["index"].as_u64().unwrap_or(0) as usize;
                        let block_type = data["content_block"]["type"]
                            .as_str()
                            .unwrap_or("text")
                            .to_string();
                        current_block_type = block_type.clone();
                        if current_block_type == "thinking" {
                            current_thinking = String::new();
                        } else {
                            current_text = String::new();
                        }
                        on_event(&StreamEvent::BlockStart { index, block_type });
                    }
                    "content_block_delta" => {
                        let delta_type = data["delta"]["type"].as_str().unwrap_or_default();
                        match delta_type {
                            "thinking_delta" => {
                                let text = data["delta"]["thinking"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string();
                                current_thinking.push_str(&text);
                                on_event(&StreamEvent::ThinkingDelta(text));
                            }
                            "text_delta" => {
                                let text = data["delta"]["text"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string();
                                current_text.push_str(&text);
                                on_event(&StreamEvent::TextDelta(text));
                            }
                            _ => {}
                        }
                    }
                    "content_block_stop" => {
                        let index = data["index"].as_u64().unwrap_or(0) as usize;
                        match current_block_type.as_str() {
                            "thinking" => {
                                content_blocks.push(ContentBlock::Thinking {
                                    thinking: std::mem::take(&mut current_thinking),
                                    signature: String::new(),
                                });
                            }
                            _ => {
                                content_blocks.push(ContentBlock::Text {
                                    text: std::mem::take(&mut current_text),
                                    citations: Vec::new(),
                                });
                            }
                        }
                        on_event(&StreamEvent::BlockStop { index });
                    }
                    "message_delta" => {
                        if let Some(sr) = data["delta"]["stop_reason"].as_str() {
                            stop_reason =
                                serde_json::from_value(serde_json::Value::String(sr.to_string()))
                                    .ok();
                        }
                        if let Some(u) = data.get("usage") {
                            usage.output_tokens = u["output_tokens"].as_u64().unwrap_or(0) as u32;
                        }
                        on_event(&StreamEvent::MessageDelta {
                            stop_reason: data["delta"]["stop_reason"].as_str().map(String::from),
                            usage: usage.clone(),
                        });
                    }
                    "message_stop" => {
                        done = true;
                        break;
                    }
                    _ => {
                        debug!("Unknown SSE event type: {evt}");
                    }
                }
            }
        }

        Ok(Message {
            id: message_id,
            content: content_blocks,
            model: model_name,
            role: Role::Assistant,
            stop_reason,
            usage,
            message_type: "message".to_string(),
        })
    }
}
