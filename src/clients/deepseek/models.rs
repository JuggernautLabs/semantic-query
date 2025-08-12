#[derive(Debug, Clone, PartialEq)]
pub enum DeepSeekModel {
    Chat,        // "deepseek-chat"
    Reasoner,    // "deepseek-reasoner"
    Override(String),
}

impl Default for DeepSeekModel {
    fn default() -> Self { DeepSeekModel::Chat }
}

impl DeepSeekModel {
    pub fn id(&self) -> &str {
        match self {
            DeepSeekModel::Chat => "deepseek-chat",
            DeepSeekModel::Reasoner => "deepseek-reasoner",
            DeepSeekModel::Override(s) => s.as_str(),
        }
    }
}

