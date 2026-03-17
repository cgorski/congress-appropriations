use super::types::*;

pub struct MessageBuilder {
    model: String,
    messages: Vec<MessageParam>,
    max_tokens: u32,
    system: Option<SystemContent>,
    thinking: Option<ThinkingConfig>,
    temperature: Option<f32>,
    stop_sequences: Option<Vec<String>>,
    stream: Option<bool>,
}

impl MessageBuilder {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            max_tokens: 4096,
            system: None,
            thinking: None,
            temperature: None,
            stop_sequences: None,
            stream: None,
        }
    }

    pub fn system(mut self, text: impl Into<String>) -> Self {
        self.system = Some(SystemContent::Text(text.into()));
        self
    }

    /// System prompt with cache control for prompt caching
    pub fn system_cached(mut self, text: impl Into<String>) -> Self {
        self.system = Some(SystemContent::Blocks(vec![SystemBlock::Text {
            text: text.into(),
            cache_control: Some(CacheControl::ephemeral()),
        }]));
        self
    }

    pub fn user(mut self, text: impl Into<String>) -> Self {
        self.messages.push(MessageParam {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        });
        self
    }

    pub fn user_with_blocks(mut self, blocks: Vec<ContentBlockParam>) -> Self {
        self.messages.push(MessageParam {
            role: Role::User,
            content: MessageContent::Blocks(blocks),
        });
        self
    }

    pub fn user_with_document(
        mut self,
        text: impl Into<String>,
        doc_text: impl Into<String>,
        title: impl Into<String>,
    ) -> Self {
        self.messages.push(MessageParam {
            role: Role::User,
            content: MessageContent::Blocks(vec![
                ContentBlockParam::Document {
                    source: DocumentSource::plain_text(doc_text),
                    title: Some(title.into()),
                    cache_control: Some(CacheControl::ephemeral()),
                    citations: None,
                },
                ContentBlockParam::Text {
                    text: text.into(),
                    cache_control: None,
                },
            ]),
        });
        self
    }

    pub fn assistant(mut self, text: impl Into<String>) -> Self {
        self.messages.push(MessageParam {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
        });
        self
    }

    pub fn message(mut self, param: MessageParam) -> Self {
        self.messages.push(param);
        self
    }

    pub fn messages(mut self, params: impl IntoIterator<Item = MessageParam>) -> Self {
        self.messages.extend(params);
        self
    }

    pub fn max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = tokens;
        self
    }

    pub fn thinking(mut self, config: ThinkingConfig) -> Self {
        self.thinking = Some(config);
        self
    }

    pub fn thinking_enabled(mut self, budget_tokens: u32) -> Self {
        self.thinking = Some(ThinkingConfig::Enabled { budget_tokens });
        self
    }

    pub fn thinking_adaptive(mut self) -> Self {
        self.thinking = Some(ThinkingConfig::Adaptive);
        self
    }

    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn stop_sequences(mut self, sequences: Vec<String>) -> Self {
        self.stop_sequences = Some(sequences);
        self
    }

    pub fn stream(mut self, enable: bool) -> Self {
        self.stream = Some(enable);
        self
    }

    pub fn build(self) -> CreateMessageRequest {
        CreateMessageRequest {
            model: self.model,
            messages: self.messages,
            max_tokens: self.max_tokens,
            system: self.system,
            thinking: self.thinking,
            temperature: self.temperature,
            stop_sequences: self.stop_sequences,
            stream: self.stream,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_builder() {
        let req = MessageBuilder::new("claude-opus-4-6")
            .system("You are helpful.")
            .user("Hello")
            .max_tokens(1024)
            .build();

        assert_eq!(req.model, "claude-opus-4-6");
        assert_eq!(req.max_tokens, 1024);
        assert_eq!(req.messages.len(), 1);
        assert!(req.system.is_some());
        assert!(req.thinking.is_none());
        assert!(req.temperature.is_none());
        assert!(req.stop_sequences.is_none());
    }

    #[test]
    fn builder_with_thinking() {
        let req = MessageBuilder::new("claude-opus-4-6")
            .user("Think hard about this.")
            .thinking_adaptive()
            .max_tokens(128000)
            .build();

        assert!(req.thinking.is_some());
        match req.thinking.as_ref().unwrap() {
            ThinkingConfig::Adaptive => {}
            other => panic!("expected Adaptive, got {other:?}"),
        }
    }

    #[test]
    fn builder_with_document() {
        let req = MessageBuilder::new("claude-opus-4-6")
            .user_with_document("Summarize this", "Document content here", "My Doc")
            .max_tokens(2048)
            .build();

        assert_eq!(req.messages.len(), 1);
        match &req.messages[0].content {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 2);
            }
            _ => panic!("expected Blocks content"),
        }
    }

    #[test]
    fn builder_with_cached_system() {
        let req = MessageBuilder::new("claude-opus-4-6")
            .system_cached("Cached system prompt")
            .user("Hello")
            .build();

        match &req.system {
            Some(SystemContent::Blocks(blocks)) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    SystemBlock::Text {
                        cache_control,
                        text,
                    } => {
                        assert_eq!(text, "Cached system prompt");
                        assert!(cache_control.is_some());
                    }
                }
            }
            _ => panic!("expected Blocks system content"),
        }
    }

    #[test]
    fn serialization_skips_none_fields() {
        let req = MessageBuilder::new("claude-opus-4-6").user("Hi").build();

        let json = serde_json::to_value(&req).unwrap();
        assert!(!json.as_object().unwrap().contains_key("system"));
        assert!(!json.as_object().unwrap().contains_key("thinking"));
        assert!(!json.as_object().unwrap().contains_key("temperature"));
        assert!(!json.as_object().unwrap().contains_key("stop_sequences"));
        assert!(!json.as_object().unwrap().contains_key("stream"));
    }
}
