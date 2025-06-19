use crate::core::{LowLevelClient};
use crate::config::KeyFromEnv;
use crate::error::{AIError, ClaudeError};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug, instrument};

#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: ClaudeMessageContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ClaudeMessageContent {
    Simple(String),
    Structured(Vec<ClaudeContentBlock>),
}

#[derive(Debug, Serialize)]
struct ClaudeContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

#[derive(Debug, Serialize)]
struct CacheControl {
    #[serde(rename = "type")]
    cache_type: String,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContent {
    text: String,
}

/// Configuration for Claude client
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub enable_caching: bool,
    pub cache_threshold: usize,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-3-5-sonnet-20241022".to_string(),
            max_tokens: 4096,
            enable_caching: true,
            cache_threshold: 3000,
        }
    }
}

#[derive(Clone)]
pub struct ClaudeClient {
    config: ClaudeConfig,
    client: Client,
}

impl KeyFromEnv for ClaudeClient {
    const KEY_NAME: &'static str = "ANTHROPIC_API_KEY";
}

impl Default for ClaudeClient {
    fn default() -> Self {
        let api_key = Self::find_key_with_user();
        let mut config = ClaudeConfig::default();
        config.api_key = api_key;
            
        info!(model = %config.model, "Creating new Claude client with caching support");
        Self {
            config,
            client: Client::new(),
        }
    }
}

impl ClaudeClient {

    /// Create a new Claude client with full configuration
    pub fn new(config: ClaudeConfig) -> Self {
        info!(model = %config.model, "Creating new Claude client");
        Self {
            config,
            client: Client::new(),
        }
    }

   
}

#[async_trait]
impl LowLevelClient for ClaudeClient {
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len(), model = %self.config.model))]
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        debug!(model = %self.config.model, prompt_len = prompt.len(), "Preparing Claude API request");
        
        let content = if self.config.enable_caching && prompt.len() > self.config.cache_threshold {
            // Split prompt for optimal caching
            
            ClaudeMessageContent::Structured(vec![
                // Base instructions - cacheable
                ClaudeContentBlock {
                    block_type: "text".to_string(),
                    text: prompt,
                    cache_control: Some(CacheControl {
                        cache_type: "ephemeral".to_string(),
                    }),
                },
            ])
        } else {
            // Fallback to simple content for short prompts
            debug!("Using simple prompt (too short for caching or caching disabled)");
            ClaudeMessageContent::Simple(prompt)
        };
        
        let request = ClaudeRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content,
            }],
        };
        
        debug!("Sending request to Claude API");
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "HTTP request failed");
                AIError::Claude(ClaudeError::Http(e.to_string()))
            })?;
            
        debug!(status = %response.status(), "Received response from Claude API");
            
        if response.status() == 429 {
            warn!("Claude API rate limit exceeded");
            return Err(AIError::Claude(ClaudeError::RateLimit));
        }
        
        if response.status() == 401 {
            error!("Claude API authentication failed");
            return Err(AIError::Claude(ClaudeError::Authentication));
        }
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(status = %status, error = %error_text, "Claude API error");
            return Err(AIError::Claude(ClaudeError::Api(error_text)));
        }
        
        let claude_response: ClaudeResponse = response
            .json()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to parse Claude response JSON");
                AIError::Claude(ClaudeError::Http(e.to_string()))
            })?;
            
        debug!(content_count = claude_response.content.len(), "Parsed Claude response");
            
        let result = claude_response
            .content
            .first()
            .map(|content| content.text.clone())
            .ok_or_else(|| {
                error!("No content in Claude response");
                AIError::Claude(ClaudeError::Api("No content in response".to_string()))
            });
            
        match &result {
            Ok(text) => info!(response_len = text.len(), "Successfully received Claude response"),
            Err(e) => error!(error = %e, "Failed to extract content from Claude response"),
        }
        
        result
    }
    fn clone_box(&self) -> Box<dyn LowLevelClient>{
        Box::new(self.clone())
    }

}