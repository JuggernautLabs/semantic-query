#[derive(Debug, Clone, PartialEq)]
pub enum OpenAIModel {
    Gpt4o,
    Gpt4oMini,
    Gpt41,
    Gpt41Mini,
    Gpt35Turbo,
    O3Mini,
    Override(String),
}

impl Default for OpenAIModel {
    fn default() -> Self { Self::Gpt4oMini }
}

impl OpenAIModel {
    pub fn id(&self) -> &str {
        match self {
            Self::Gpt4o => "gpt-4o",
            Self::Gpt4oMini => "gpt-4o-mini",
            Self::Gpt41 => "gpt-4.1",
            Self::Gpt41Mini => "gpt-4.1-mini",
            Self::Gpt35Turbo => "gpt-3.5-turbo",
            Self::O3Mini => "o3-mini",
            Self::Override(s) => s.as_str(),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Gpt4o => "OpenAI GPT-4o",
            Self::Gpt4oMini => "OpenAI GPT-4o Mini",
            Self::Gpt41 => "OpenAI GPT-4.1",
            Self::Gpt41Mini => "OpenAI GPT-4.1 Mini",
            Self::Gpt35Turbo => "OpenAI GPT-3.5 Turbo",
            Self::O3Mini => "OpenAI o3-mini",
            Self::Override(_) => "OpenAI (override)",
        }
    }
}

