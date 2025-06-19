use serde::{Deserialize, Serialize};

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
    pub api_key: String,
    pub model: String,
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
            api_key: String::new(),
            model: "claude-3-5-haiku-20241022".to_string(),
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
    pub fn anthropic(api_key: String, model: String) -> Self {
        Self {
            provider: Provider::Anthropic,
            api_key,
            model,
            ..Default::default()
        }
    }

    pub fn bedrock(aws_region: String, model: String) -> Self {
        Self {
            provider: Provider::AwsBedrock,
            model,
            aws_region: Some(aws_region),
            ..Default::default()
        }
    }

    pub fn vertex(project_id: String, location: String, model: String) -> Self {
        Self {
            provider: Provider::GcpVertex,
            model,
            gcp_project_id: Some(project_id),
            gcp_location: Some(location),
            ..Default::default()
        }
    }

    pub fn get_model_for_provider(&self) -> String {
        match self.provider {
            Provider::Anthropic => self.model.clone(),
            Provider::AwsBedrock => {
                // Convert Anthropic model names to Bedrock format if needed
                match self.model.as_str() {
                    "claude-opus-4-20250514" => "anthropic.claude-opus-4-20250514-v1:0".to_string(),
                    "claude-sonnet-4-20250514" => "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
                    "claude-3-7-sonnet-20250219" => "anthropic.claude-3-7-sonnet-20250219-v1:0".to_string(),
                    "claude-3-5-haiku-20241022" => "anthropic.claude-3-5-haiku-20241022-v1:0".to_string(),
                    "claude-3-5-sonnet-20241022" => "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
                    "claude-3-5-sonnet-20240620" => "anthropic.claude-3-5-sonnet-20240620-v1:0".to_string(),
                    "claude-3-opus-20240229" => "anthropic.claude-3-opus-20240229-v1:0".to_string(),
                    "claude-3-sonnet-20240229" => "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
                    "claude-3-haiku-20240307" => "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
                    _ => self.model.clone(),
                }
            }
            Provider::GcpVertex => {
                // Convert Anthropic model names to Vertex format if needed
                match self.model.as_str() {
                    "claude-opus-4-20250514" => "claude-opus-4@20250514".to_string(),
                    "claude-sonnet-4-20250514" => "claude-sonnet-4@20250514".to_string(),
                    "claude-3-7-sonnet-20250219" => "claude-3-7-sonnet@20250219".to_string(),
                    "claude-3-5-haiku-20241022" => "claude-3-5-haiku@20241022".to_string(),
                    "claude-3-5-sonnet-20241022" => "claude-3-5-sonnet-v2@20241022".to_string(),
                    "claude-3-5-sonnet-20240620" => "claude-3-5-sonnet@20240620".to_string(),
                    "claude-3-opus-20240229" => "claude-3-opus@20240229".to_string(),
                    "claude-3-sonnet-20240229" => "claude-3-sonnet@20240229".to_string(),
                    "claude-3-haiku-20240307" => "claude-3-haiku@20240307".to_string(),
                    _ => self.model.clone(),
                }
            }
        }
    }
}