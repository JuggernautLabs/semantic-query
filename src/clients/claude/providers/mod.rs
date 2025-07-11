pub mod anthropic;
pub mod bedrock;
pub mod vertex;

pub use anthropic::*;
pub use bedrock::*;
pub use vertex::*;

use crate::error::AIError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::config::ClaudeConfig;

#[derive(Debug, Serialize)]
pub struct ClaudeRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: ClaudeMessageContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ClaudeMessageContent {
    Simple(String),
    Structured(Vec<ClaudeContentBlock>),
}

#[derive(Debug, Serialize)]
pub struct ClaudeContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Serialize)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeContent {
    pub text: String,
}

impl ClaudeRequest {
    pub fn new(prompt: String, config: &ClaudeConfig) -> Self {
        let content = if config.enable_caching && prompt.len() > config.cache_threshold {
            ClaudeMessageContent::Structured(vec![
                ClaudeContentBlock {
                    block_type: "text".to_string(),
                    text: prompt,
                    cache_control: Some(CacheControl {
                        cache_type: "ephemeral".to_string(),
                    }),
                },
            ])
        } else {
            ClaudeMessageContent::Simple(prompt)
        };

        Self {
            model: config.get_model_for_provider(),
            max_tokens: config.max_tokens,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content,
            }],
        }
    }
}

#[async_trait]
pub trait ClaudeProvider: Send + Sync {
    async fn call_api(&self, request: &ClaudeRequest) -> Result<String, AIError>;
}