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
    #[cfg(feature = "anthropic")] 
    Anthropic(AnthropicProvider),
    #[cfg(feature = "bedrock")] 
    Bedrock(BedrockProvider),
    #[cfg(feature = "vertex")] 
    Vertex(VertexProvider),
}

impl ClaudeClientProvider {
    async fn call_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        match self {
            #[cfg(feature = "anthropic")] 
            Self::Anthropic(provider) => provider.call_api(request).await,
            #[cfg(feature = "bedrock")] 
            Self::Bedrock(provider) => provider.call_api(request).await,
            #[cfg(feature = "vertex")] 
            Self::Vertex(provider) => provider.call_api(request).await,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ClaudeClient {
    provider: ClaudeClientProvider,
    config: ClaudeConfig,
}

impl KeyFromEnv for ClaudeConfig {
    const KEY_NAME: &'static str = "ANTHROPIC_API_KEY";
}

impl Default for ClaudeClient {
    fn default() -> Self {
        let config = ClaudeConfig::anthropic(ClaudeConfig::find_key().unwrap_or(String::new()), ClaudeModel::Haiku35);
        Self::new(config)
    }
}

impl ClaudeClient {
    pub fn new(config: ClaudeConfig) -> Self {
        let provider = match config.provider {
            #[cfg(feature = "anthropic")] 
            Provider::Anthropic => ClaudeClientProvider::Anthropic(AnthropicProvider::new(config.clone())),
            #[cfg(feature = "bedrock")] 
            Provider::AwsBedrock => ClaudeClientProvider::Bedrock(BedrockProvider::new(config.clone())),
            #[cfg(feature = "vertex")] 
            Provider::GcpVertex => ClaudeClientProvider::Vertex(VertexProvider::new(config.clone())),
            #[allow(unreachable_patterns)]
            _ => panic!("Requested provider is not enabled via features"),
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
