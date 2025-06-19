use crate::core::{LowLevelClient};
use crate::error::{AIError, ClaudeError};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug, instrument};
use std::env;

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

#[derive(Clone)]
pub struct ClaudeClient {
    api_key: String,
    client: Client,
    model: String,
}

impl Default for ClaudeClient {
    fn default() -> Self {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY environment variable must be set");
            
        info!(model = "claude-3-5-sonnet-20241022", "Creating new Claude client with caching support");
        Self {
            api_key,
            client: Client::new(),
            model: "claude-3-5-sonnet-20241022".to_string(),
        }
    }
}

impl ClaudeClient {

        /// Create a new DeepSeek client by reading DEEPSEEK_API_KEY from environment/.env
        pub fn new() -> Result<Self, AIError> {
            // Try to load .env file (silently fail if not found)
            let _ = dotenvy::dotenv();
            
            let api_key = env::var("ANTHROPIC_API_KEY")
                .map_err(|_| ClaudeError::Authentication)?;
                
            info!(model = "deepseek-chat", "Creating new DeepSeek client");
            Ok(Self {
                api_key,
                client: Client::new(),
                model: "deepseek-chat".to_string(),
            })
        }
    /// Create a new Claude client with an explicit API key
    pub fn with_api_key(api_key: String) -> Self {
        info!(model = "claude-3-5-sonnet-20241022", "Creating new Claude client with explicit API key");
        Self {
            api_key,
            client: Client::new(),
            model: "claude-3-5-sonnet-20241022".to_string(), // Use Sonnet for better caching
        }
    }
    
    pub fn with_model(mut self, model: String) -> Self {
        info!(model = %model, "Setting Claude model");
        self.model = model;
        self
    }

   
}

#[async_trait]
impl LowLevelClient for ClaudeClient {
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len(), model = %self.model))]
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        debug!(model = %self.model, prompt_len = prompt.len(), "Preparing Claude API request");
        
        let content = if  prompt.len() > 3000 {
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
            model: self.model.clone(),
            max_tokens: 4096,
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content,
            }],
        };
        
        debug!("Sending request to Claude API");
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
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