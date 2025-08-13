#[derive(Debug, Clone, PartialEq)]
pub enum OpenAIModel {
    // Next-gen ChatGPT model
    Gpt5,
    Gpt4o,
    Gpt4oMini,
    Gpt4_1,
    Gpt4_1Mini,
    Gpt35Turbo,
    O3Mini,
    O1,
    O1Mini,
    Override(String),
}

impl OpenAIModel {
    pub fn id(&self) -> &str {
        match self {
            OpenAIModel::Gpt5 => "gpt-5",
            OpenAIModel::Gpt4o => "gpt-4o",
            OpenAIModel::Gpt4oMini => "gpt-4o-mini",
            OpenAIModel::Gpt4_1 => "gpt-4.1",
            OpenAIModel::Gpt4_1Mini => "gpt-4.1-mini",
            OpenAIModel::Gpt35Turbo => "gpt-3.5-turbo",
            OpenAIModel::O3Mini => "o3-mini",
            OpenAIModel::O1 => "o1",
            OpenAIModel::O1Mini => "o1-mini",
            OpenAIModel::Override(s) => s.as_str(),
        }
    }
}
