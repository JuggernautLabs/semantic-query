use crate::config::KeyFromEnv;

use super::models::ClaudeModel;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provider {
    #[cfg(feature = "anthropic")] 
    Anthropic,
    #[cfg(feature = "bedrock")] 
    AwsBedrock,
}

impl Default for Provider {
    fn default() -> Self {
        Self::Anthropic
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub provider: Provider,
    pub model: ClaudeModel,
    pub api_key: String,
    pub max_tokens: u32,
    pub enable_caching: bool,
    pub cache_threshold: usize,
    // AWS Bedrock specific
    pub aws_region: Option<String>,
    pub aws_access_key_id: Option<String>,
    pub aws_secret_access_key: Option<String>,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            provider: Provider::default(),
            model: ClaudeModel::default(),
            api_key: Self::find_key().unwrap_or(String::new()),

            max_tokens: 4096,
            enable_caching: true,
            cache_threshold: 3000,
            aws_region: None,
            aws_access_key_id: None,
            aws_secret_access_key: None,
        }
    }
}

impl ClaudeConfig {
    #[must_use]
    pub fn new(provider: Provider, model: ClaudeModel) -> Self {
        Self {
            provider,
            model,
            ..Default::default()
        }
    }

    #[cfg(feature = "anthropic")] 
    #[must_use]
    pub fn anthropic(api_key: String, model: ClaudeModel) -> Self {
        Self {
            provider: Provider::Anthropic,
            model,
            api_key,
            ..Default::default()
        }
    }

    #[cfg(feature = "bedrock")] 
    #[must_use]
    pub fn bedrock(aws_region: String, model: ClaudeModel) -> Self {
        Self {
            provider: Provider::AwsBedrock,
            model,
            aws_region: Some(aws_region),
            ..Default::default()
        }
    }

    #[must_use]
    pub fn get_model_for_provider(&self) -> String {
        self.model.model_id_for_provider(&self.provider).to_string()
    }

    #[must_use]
    pub const fn with_provider(mut self, provider: Provider) -> Self {
        self.provider = provider;
        self
    }

    #[must_use]
    pub const fn with_model(mut self, model: ClaudeModel) -> Self {
        self.model = model;
        self
    }
}
