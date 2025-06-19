use crate::core::{LowLevelClient};
use crate::error::{AIError, DeepSeekError};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error, debug, instrument};
use std::env;

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

#[derive(Clone)]
pub struct DeepSeekClient {
    api_key: String,
    client: Client,
    model: String,
}

impl Default for DeepSeekClient {
    fn default() -> Self {
        let _ = dotenvy::dotenv();

        let api_key = env::var("DEEPSEEK_API_KEY")
            .expect("DEEPSEEK_API_KEY environment variable must be set");
            
        info!(model = "deepseek-chat", "Creating new DeepSeek client");
        Self {
            api_key,
            client: Client::new(),
            model: "deepseek-chat".to_string(),
        }
    }
}


impl DeepSeekClient {
    /// Create a new DeepSeek client by reading DEEPSEEK_API_KEY from environment/.env
    pub fn new() -> Result<Self, AIError> {
        // Try to load .env file (silently fail if not found)
        let _ = dotenvy::dotenv();
        
        let api_key = env::var("DEEPSEEK_API_KEY")
            .map_err(|_| DeepSeekError::Authentication)?;
            
        info!(model = "deepseek-chat", "Creating new DeepSeek client");
        Ok(Self {
            api_key,
            client: Client::new(),
            model: "deepseek-chat".to_string(),
        })
    }
    
    /// Create a new DeepSeek client with an explicit API key
    pub fn with_api_key(api_key: String) -> Self {
        info!(model = "deepseek-chat", "Creating new DeepSeek client with explicit API key");
        Self {
            api_key,
            client: Client::new(),
            model: "deepseek-chat".to_string(),
        }
    }
    
    pub fn with_model(mut self, model: String) -> Self {
        info!(model = %model, "Setting DeepSeek model");
        self.model = model;
        self
    }
}

#[async_trait]
impl LowLevelClient for DeepSeekClient {
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len(), model = %self.model))]
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        debug!(model = %self.model, prompt_len = prompt.len(), "Preparing DeepSeek API request");
        
        let request = DeepSeekRequest {
            model: self.model.clone(),
            messages: vec![DeepSeekMessage {
                role: "user".to_string(),
                content: prompt,
            }],
            max_tokens: 4096,
            temperature: 0.3,
        };
        
        debug!("Sending request to DeepSeek API");
        let response = self
            .client
            .post("https://api.deepseek.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
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