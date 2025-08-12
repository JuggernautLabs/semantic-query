#[derive(Debug, Clone, PartialEq)]
pub enum DeepSeekModel {
    Chat,
    Reasoner,
    Override(String),
}

impl Default for DeepSeekModel {
    fn default() -> Self { Self::Chat }
}

impl DeepSeekModel {
    pub fn id(&self) -> &str {
        match self {
            Self::Chat => "deepseek-chat",
            Self::Reasoner => "deepseek-reasoner",
            Self::Override(s) => s.as_str(),
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Chat => "DeepSeek Chat",
            Self::Reasoner => "DeepSeek Reasoner",
            Self::Override(_) => "DeepSeek (override)",
        }
    }
}

