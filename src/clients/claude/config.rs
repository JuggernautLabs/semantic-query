use crate::config::KeyFromEnv;

use super::models::ClaudeModel;

#[derive(Debug, Clone, PartialEq)]
pub enum Provider {
    Anthropic,
    AwsBedrock,
    GcpVertex,
}

impl Default for Provider {
    fn default() -> Self {
        Self::Anthropic
    }
}

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
    // GCP Vertex specific
    pub gcp_project_id: Option<String>,
    pub gcp_location: Option<String>,
    pub gcp_credentials_path: Option<String>,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            provider: Provider::default(),
            model: ClaudeModel::default(),
            api_key: ClaudeConfig::find_key().unwrap_or(String::new()),

            max_tokens: 4096,
            enable_caching: true,
            cache_threshold: 3000,
            aws_region: None,
            aws_access_key_id: None,
            aws_secret_access_key: None,
            gcp_project_id: None,
            gcp_location: None,
            gcp_credentials_path: None,
        }
    }
}

impl ClaudeConfig {
    pub fn new(provider: Provider, model: ClaudeModel) -> Self {
        Self {
            provider,
            model,
            ..Default::default()
        }
    }

    pub fn anthropic(api_key: String, model: ClaudeModel) -> Self {
        Self {
            provider: Provider::Anthropic,
            model,
            api_key,
            ..Default::default()
        }
    }

    pub fn bedrock(aws_region: String, model: ClaudeModel) -> Self {
        Self {
            provider: Provider::AwsBedrock,
            model,
            aws_region: Some(aws_region),
            ..Default::default()
        }
    }

    pub fn vertex(project_id: String, location: String, model: ClaudeModel) -> Self {
        Self {
            provider: Provider::GcpVertex,
            model,
            gcp_project_id: Some(project_id),
            gcp_location: Some(location),
            ..Default::default()
        }
    }

    pub fn get_model_for_provider(&self) -> String {
        self.model.model_id_for_provider(&self.provider).to_string()
    }

    pub fn with_provider(mut self, provider: Provider) -> Self {
        self.provider = provider;
        self
    }

    pub fn with_model(mut self, model: ClaudeModel) -> Self {
        self.model = model;
        self
    }
}