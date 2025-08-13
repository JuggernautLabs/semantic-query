#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeModel {
    // Claude 4 Models
    Opus4,
    Sonnet4,
    
    // Claude 3.7 Models
    Sonnet37,
    
    // Claude 3.5 Models
    Haiku35,
    Sonnet35V2,
    Sonnet35,
    
    // Claude 3 Models
    Opus3,
    Sonnet3,
    Haiku3,
}

impl Default for ClaudeModel {
    fn default() -> Self {
        Self::Haiku35
    }
}

impl ClaudeModel {
    #[must_use]
    pub const fn anthropic_model_id(&self) -> &'static str {
        match self {
            Self::Opus4 => "claude-opus-4-20250514",
            Self::Sonnet4 => "claude-sonnet-4-20250514",
            Self::Sonnet37 => "claude-3-7-sonnet-20250219",
            Self::Haiku35 => "claude-3-5-haiku-20241022",
            Self::Sonnet35V2 => "claude-3-5-sonnet-20241022",
            Self::Sonnet35 => "claude-3-5-sonnet-20240620",
            Self::Opus3 => "claude-3-opus-20240229",
            Self::Sonnet3 => "claude-3-sonnet-20240229",
            Self::Haiku3 => "claude-3-haiku-20240307",
        }
    }

    #[must_use]
    pub const fn bedrock_model_id(&self) -> &'static str {
        match self {
            Self::Opus4 => "anthropic.claude-opus-4-20250514-v1:0",
            Self::Sonnet4 => "anthropic.claude-sonnet-4-20250514-v1:0",
            Self::Sonnet37 => "anthropic.claude-3-7-sonnet-20250219-v1:0",
            Self::Haiku35 => "anthropic.claude-3-5-haiku-20241022-v1:0",
            Self::Sonnet35V2 => "anthropic.claude-3-5-sonnet-20241022-v2:0",
            Self::Sonnet35 => "anthropic.claude-3-5-sonnet-20240620-v1:0",
            Self::Opus3 => "anthropic.claude-3-opus-20240229-v1:0",
            Self::Sonnet3 => "anthropic.claude-3-sonnet-20240229-v1:0",
            Self::Haiku3 => "anthropic.claude-3-haiku-20240307-v1:0",
        }
    }

    #[must_use]
    pub const fn model_id_for_provider(&self, provider: &super::config::Provider) -> &'static str {
        match provider {
            #[cfg(feature = "anthropic")] 
            super::config::Provider::Anthropic => self.anthropic_model_id(),
            #[cfg(feature = "bedrock")] 
            super::config::Provider::AwsBedrock => self.bedrock_model_id(),
        }
    }

    #[must_use]
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Opus4 => "Claude 4 Opus",
            Self::Sonnet4 => "Claude 4 Sonnet", 
            Self::Sonnet37 => "Claude 3.7 Sonnet",
            Self::Haiku35 => "Claude 3.5 Haiku",
            Self::Sonnet35V2 => "Claude 3.5 Sonnet v2",
            Self::Sonnet35 => "Claude 3.5 Sonnet",
            Self::Opus3 => "Claude 3 Opus",
            Self::Sonnet3 => "Claude 3 Sonnet",
            Self::Haiku3 => "Claude 3 Haiku",
        }
    }
}
