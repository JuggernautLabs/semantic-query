use crate::config::KeyFromEnv;
use crate::error::{AIError, ClaudeError};
use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, error, info, instrument, warn};

use super::{ClaudeProvider, ClaudeRequest, ClaudeResponse};
use crate::clients::claude::config::ClaudeConfig;

#[derive(Clone, Debug)]
pub struct AnthropicProvider {
    config: ClaudeConfig,
    client: Client,
}

impl KeyFromEnv for AnthropicProvider {
    const KEY_NAME: &'static str = "ANTHROPIC_API_KEY";
}

impl AnthropicProvider {
    pub fn new(config: ClaudeConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub fn default_with_key() -> Self {
        let api_key = Self::find_key_with_user();
        let mut config = ClaudeConfig::default();
        config.api_key = api_key;
        Self::new(config)
    }
}

#[async_trait]
impl ClaudeProvider for AnthropicProvider {
    #[instrument(skip(self, request), fields(model = %request.model))]
    async fn call_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        debug!(model = %request.model, "Preparing Anthropic API request");

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "HTTP request failed");
                AIError::Claude(ClaudeError::Http(e.to_string()))
            })?;

        debug!(status = %response.status(), "Received response from Anthropic API");

        if response.status() == 429 {
            warn!("Anthropic API rate limit exceeded");
            return Err(AIError::Claude(ClaudeError::RateLimit));
        }

        if response.status() == 401 {
            error!("Anthropic API authentication failed");
            return Err(AIError::Claude(ClaudeError::Authentication));
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(status = %status, error = %error_text, "Anthropic API error");
            return Err(AIError::Claude(ClaudeError::Api(error_text)));
        }

        let claude_response: ClaudeResponse = response.json().await.map_err(|e| {
            error!(error = %e, "Failed to parse Anthropic response JSON");
            AIError::Claude(ClaudeError::Http(e.to_string()))
        })?;

        debug!(content_count = claude_response.content.len(), "Parsed Anthropic response");

        let result = claude_response
            .content
            .first()
            .map(|content| content.text.clone())
            .ok_or_else(|| {
                error!("No content in Anthropic response");
                AIError::Claude(ClaudeError::Api("No content in response".to_string()))
            });

        match &result {
            Ok(text) => info!(response_len = text.len(), "Successfully received Anthropic response"),
            Err(e) => error!(error = %e, "Failed to extract content from Anthropic response"),
        }

        result
    }
}