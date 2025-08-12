pub mod providers;
pub mod models;
pub mod config;

pub use providers::*;
pub use models::*;
pub use config::*;

use crate::core::LowLevelClient;
use futures_util::{StreamExt, TryStreamExt};
use crate::error::AIError;
use crate::config::KeyFromEnv;
use async_trait::async_trait;

#[derive(Clone, Debug)]
pub enum ClaudeClientProvider {
    #[cfg(feature = "anthropic")] 
    Anthropic(AnthropicProvider),
    #[cfg(feature = "bedrock")] 
    Bedrock(BedrockProvider),
}

impl ClaudeClientProvider {
    async fn call_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        match self {
            #[cfg(feature = "anthropic")] 
            Self::Anthropic(provider) => provider.call_api(request).await,
            #[cfg(feature = "bedrock")] 
            Self::Bedrock(provider) => provider.call_api(request).await,
        }
    }

    async fn stream_api(&self, request: &ClaudeRequest) -> Result<std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<bytes::Bytes, AIError>> + Send>>, AIError> {
        match self {
            #[cfg(feature = "anthropic")] 
            Self::Anthropic(provider) => provider.stream_api(request).await,
            #[cfg(feature = "bedrock")] 
            Self::Bedrock(_) => Err(AIError::Claude(crate::error::ClaudeError::Api("Bedrock streaming not implemented".into()))),
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

    fn stream_raw(&self, prompt: String) -> Option<std::pin::Pin<Box<dyn futures_core::Stream<Item = Result<bytes::Bytes, AIError>> + Send>>> {
        let config = self.config.clone();
        let provider = self.provider.clone();
        let s = async_stream::try_stream! {
            let request = ClaudeRequest::new(prompt, &config);
            let mut bs = provider.stream_api(&request).await?;
            while let Some(chunk) = bs.next().await {
                let b = chunk?;
                yield b;
            }
        };
        Some(Box::pin(s.map_err(|e| e)))
    }

    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}
