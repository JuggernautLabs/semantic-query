#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DeepSeekModel {
    #[default]
    Chat,        // "deepseek-chat"
    Reasoner,    // "deepseek-reasoner"
    Override(String),
}

impl DeepSeekModel {
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Chat => "deepseek-chat",
            Self::Reasoner => "deepseek-reasoner",
            Self::Override(s) => s.as_str(),
        }
    }
}
