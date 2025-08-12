pub mod claude;
pub mod deepseek;
pub mod flexible;
pub mod mock;
pub mod chatgpt;

// Re-export only the public surface needed by consumers to avoid ambiguous glob re-exports
pub use claude::{ClaudeClient, ClaudeConfig};
pub use claude::models::ClaudeModel;
pub use deepseek::DeepSeekClient;
pub use deepseek::models::DeepSeekModel;
pub use flexible::{FlexibleClient, ClientType};
pub use mock::{MockClient, MockHandle, MockResponse, MockVoid};
pub use chatgpt::{OpenAIClient, OpenAIConfig, AzureOpenAIClient, AzureOpenAIConfig};
pub use chatgpt::models::OpenAIModel;
