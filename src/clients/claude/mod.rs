pub mod providers;
pub mod models;
pub mod config;

pub use providers::*;
pub use models::*;
pub use config::*;

use crate::core::LowLevelClient;
use crate::error::AIError;
use crate::config::KeyFromEnv;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub enum ClaudeClientProvider {
    Anthropic(AnthropicProvider),
    Bedrock(BedrockProvider),
    Vertex(VertexProvider),
}

impl ClaudeClientProvider {
    async fn call_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        match self {
            Self::Anthropic(provider) => provider.call_api(request).await,
            Self::Bedrock(provider) => provider.call_api(request).await,
            Self::Vertex(provider) => provider.call_api(request).await,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClaudeClient {
    provider: ClaudeClientProvider,
    config: ClaudeConfig,
}

impl KeyFromEnv for ClaudeClient {
    const KEY_NAME: &'static str = "ANTHROPIC_API_KEY";
}

impl Default for ClaudeClient {
    fn default() -> Self {
        let api_key = Self::find_key_with_user();
        let config = ClaudeConfig::anthropic(api_key, ClaudeModel::Haiku35);
        Self::new(config)
    }
}

impl ClaudeClient {
    pub fn new(config: ClaudeConfig) -> Self {
        let provider = match config.provider {
            Provider::Anthropic => ClaudeClientProvider::Anthropic(AnthropicProvider::new(config.clone())),
            Provider::AwsBedrock => ClaudeClientProvider::Bedrock(BedrockProvider::new(config.clone())),
            Provider::GcpVertex => ClaudeClientProvider::Vertex(VertexProvider::new(config.clone())),
        };

        Self { provider, config }
    }
}

#[async_trait]
impl LowLevelClient for ClaudeClient {
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        let request = ClaudeRequest::new(prompt, &self.config);
        self.provider.call_api(&request).await
    }

    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}