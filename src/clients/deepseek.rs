use crate::core::{LowLevelClient};
use crate::config::KeyFromEnv;
use crate::error::{AIError, DeepSeekError};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug, instrument};

#[derive(Debug, Serialize)]
struct DeepSeekRequest {
    model: String,
    messages: Vec<DeepSeekMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct DeepSeekMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct DeepSeekResponse {
    choices: Vec<DeepSeekChoice>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChoice {
    message: DeepSeekResponseMessage,
}

#[derive(Debug, Deserialize)]
struct DeepSeekResponseMessage {
    content: String,
}

/// Configuration for DeepSeek client
#[derive(Debug, Clone)]
pub struct DeepSeekConfig {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for DeepSeekConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "deepseek-chat".to_string(),
            max_tokens: 4096,
            temperature: 0.3,
        }
    }
}

#[derive(Clone)]
pub struct DeepSeekClient {
    config: DeepSeekConfig,
    client: Client,
}

impl KeyFromEnv for DeepSeekClient {
    const KEY_NAME: &'static str = "DEEPSEEK_API_KEY";
}

impl Default for DeepSeekClient {
    fn default() -> Self {
        let api_key = Self::find_key_with_user();
        let mut config = DeepSeekConfig::default();
        config.api_key = api_key;
            
        info!(model = %config.model, "Creating new DeepSeek client");
        Self {
            config,
            client: Client::new(),
        }
    }
}


impl DeepSeekClient {
    /// Create a new DeepSeek client with full configuration
    pub fn new(config: DeepSeekConfig) -> Self {
        info!(model = %config.model, "Creating new DeepSeek client");
        Self {
            config,
            client: Client::new(),
        }
    }
    
}

#[async_trait]
impl LowLevelClient for DeepSeekClient {
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len(), model = %self.config.model))]
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        debug!(model = %self.config.model, prompt_len = prompt.len(), "Preparing DeepSeek API request");
        
        let request = DeepSeekRequest {
            model: self.config.model.clone(),
            messages: vec![DeepSeekMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };
        
        debug!("Sending request to DeepSeek API");
        let response = self
            .client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "HTTP request failed");
                AIError::DeepSeek(DeepSeekError::Http(e.to_string()))
            })?;
            
        debug!(status = %response.status(), "Received response from DeepSeek API");
            
        if response.status() == 429 {
            warn!("DeepSeek API rate limit exceeded");
            return Err(AIError::DeepSeek(DeepSeekError::RateLimit));
        }
        
        if response.status() == 401 {
            error!("DeepSeek API authentication failed");
            return Err(AIError::DeepSeek(DeepSeekError::Authentication));
        }
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(status = %status, error = %error_text, "DeepSeek API error");
            return Err(AIError::DeepSeek(DeepSeekError::Api(error_text)));
        }
        
        let deepseek_response: DeepSeekResponse = response
            .json()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to parse DeepSeek response JSON");
                AIError::DeepSeek(DeepSeekError::Http(e.to_string()))
            })?;
            
        debug!(choices_count = deepseek_response.choices.len(), "Parsed DeepSeek response");
            
        let result = deepseek_response
            .choices
            .first()
            .map(|choice| choice.message.content.clone())
            .ok_or_else(|| {
                error!("No choices in DeepSeek response");
                AIError::DeepSeek(DeepSeekError::Api("No choices in response".to_string()))
            });
            
        match &result {
            Ok(text) => info!(response_len = text.len(), "Successfully received DeepSeek response"),
            Err(e) => error!(error = %e, "Failed to extract content from DeepSeek response"),
        }
        
        result
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}