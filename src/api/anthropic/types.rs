use serde::{Deserialize, Serialize};

// ─── Request Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub model: String,
    pub messages: Vec<MessageParam>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// System can be a string or array of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemContent {
    Text(String),
    Blocks(Vec<SystemBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SystemBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub control_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<String>,
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            control_type: "ephemeral".into(),
            ttl: None,
        }
    }

    pub fn ephemeral_1h() -> Self {
        Self {
            control_type: "ephemeral".into(),
            ttl: Some("1h".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    Enabled { budget_tokens: u32 },
    Disabled,
    Adaptive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageParam {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// Content can be a plain string or array of blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlockParam>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockParam {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "document")]
    Document {
        source: DocumentSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
        #[serde(skip_serializing_if = "Option::is_none")]
        citations: Option<CitationsConfig>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DocumentSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "text")]
    PlainText { media_type: String, data: String },
}

impl DocumentSource {
    pub fn plain_text(data: impl Into<String>) -> Self {
        Self::PlainText {
            media_type: "text/plain".into(),
            data: data.into(),
        }
    }

    pub fn pdf_base64(data: impl Into<String>) -> Self {
        Self::Base64 {
            media_type: "application/pdf".into(),
            data: data.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "url")]
    Url { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationsConfig {
    pub enabled: bool,
}

// ─── Response Types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub role: Role,
    pub stop_reason: Option<StopReason>,
    pub usage: Usage,
    #[serde(rename = "type")]
    pub message_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(default)]
        citations: Vec<serde_json::Value>,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String, signature: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    StopSequence,
    ToolUse,
    PauseTurn,
    Refusal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

// ─── Helper Methods ──────────────────────────────────────────────────────────

impl Message {
    /// Extract the first text content block.
    pub fn text(&self) -> Option<&str> {
        self.content.iter().find_map(|block| match block {
            ContentBlock::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
    }

    /// Extract all text content concatenated.
    pub fn all_text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Extract thinking content if present.
    pub fn thinking(&self) -> Option<&str> {
        self.content.iter().find_map(|block| match block {
            ContentBlock::Thinking { thinking, .. } => Some(thinking.as_str()),
            _ => None,
        })
    }

    /// Total tokens used.
    pub fn total_tokens(&self) -> u32 {
        self.usage.input_tokens + self.usage.output_tokens
    }
}

// ─── Streaming Types ─────────────────────────────────────────────────────────

/// Events emitted during streaming response.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Partial thinking text delta
    ThinkingDelta(String),
    /// Partial output text delta
    TextDelta(String),
    /// A content block started (index, type like "thinking" or "text")
    BlockStart { index: usize, block_type: String },
    /// A content block finished
    BlockStop { index: usize },
    /// Final usage stats from message_delta
    MessageDelta {
        stop_reason: Option<String>,
        usage: Usage,
    },
}
