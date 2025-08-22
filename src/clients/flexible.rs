use crate::clients::claude::ClaudeConfig;
use crate::clients::deepseek::DeepSeekConfig;
use crate::core::{LowLevelClient};
use bytes::Bytes;
use futures_util::StreamExt;
use crate::error::{AIError};
use crate::interceptors::{FileInterceptor, Interceptor};
use async_trait::async_trait;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio_util::io::StreamReader;
use tokio::io::AsyncRead;
use std::pin::Pin;
use std::str::FromStr;


/// Client type for lazy initialization
#[derive(Debug, Clone)]
pub enum ClientType {
    Claude,
    DeepSeek,
    ChatGPT,
    Mock,
}

impl From<ClientType> for Box<dyn LowLevelClient> {
    fn from(val: ClientType) -> Self {
        match val {
            ClientType::Claude => {
                use super::claude::ClaudeClient;
                Box::new(ClaudeClient::default())
            }
            ClientType::DeepSeek => {
                use super::deepseek::DeepSeekClient;
                Box::new(DeepSeekClient::default())
            }
            ClientType::ChatGPT => {
                // Prefer Azure OpenAI if endpoint/key present; else plain OpenAI
                let use_azure = env::var("AZURE_OPENAI_ENDPOINT").is_ok() ||
                    std::fs::read_to_string(".env").map_or(false, |c| c.contains("AZURE_OPENAI_ENDPOINT"));
                if use_azure {
                    use super::chatgpt::AzureOpenAIClient;
                    Box::new(AzureOpenAIClient::new(super::chatgpt::AzureOpenAIConfig::default()))
                } else {
                    use super::chatgpt::OpenAIClient;
                    Box::new(OpenAIClient::new(super::chatgpt::OpenAIConfig::default()))
                }
            }
            ClientType::Mock => {
                // Note: This creates a mock without a controllable handle
                // Use FlexibleClient::new_mock() if you need to control the mock
                use super::mock::MockClient;
                let (mock_client, _handle) = MockClient::new();
                // The handle is dropped here, making this mock uncontrollable
                Box::new(mock_client)
            }
        }
    }
}

impl Default for ClientType {
      /// Get the default client type based on available API keys
      fn default() -> Self {
        // Check for API keys in order of preference
        if env::var("ANTHROPIC_API_KEY").is_ok() || 
           std::fs::read_to_string(".env").map_or(false, |content| content.contains("ANTHROPIC_API_KEY")) {
            Self::Claude
        } else if env::var("DEEPSEEK_API_KEY").is_ok() || 
                 std::fs::read_to_string(".env").map_or(false, |content| content.contains("DEEPSEEK_API_KEY")) {
            Self::DeepSeek
        } else if env::var("OPENAI_API_KEY").is_ok() || 
                 std::fs::read_to_string(".env").map_or(false, |content| content.contains("OPENAI_API_KEY")) {
            Self::ChatGPT
        } else {
            Self::Mock
        }
    }
}
impl FromStr for ClientType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "deepseek" => Ok(Self::DeepSeek),
            "openai" | "chatgpt" => Ok(Self::ChatGPT),
            "mock" => Ok(Self::Mock),
            _ => Err(format!("Unknown client type: '{s}'. Supported: claude, deepseek, mock"))
        }
    }
}

impl ClientType {
    
    /// Create a mock variant that returns both the client type and a handle
    #[must_use]
    pub fn mock_with_handle() -> (Self, Arc<super::mock::MockHandle>) {
        use super::mock::MockClient;
        let (_, handle) = MockClient::new();
        (Self::Mock, handle)
    }
}


impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientType::Claude => write!(f, "Claude"),
            ClientType::DeepSeek => write!(f, "DeepSeek"),
            ClientType::ChatGPT => write!(f, "ChatGPT"),
            ClientType::Mock => write!(f, "Mock"),
        }
    }
}


#[derive(Debug)]
/// Flexible client that wraps any `LowLevelClient` and provides factory functions
pub struct FlexibleClient {
    inner: Arc<Mutex<Box<dyn LowLevelClient>>>,
    interceptor: Option<Arc<dyn Interceptor>>,
}


impl FlexibleClient {
    /// Create a new `FlexibleClient` from a client type (lazy-initialized boxed impl)
    #[must_use]
    pub fn from_type(client_type: ClientType) -> Self {
       
        Self { 
            inner: Arc::new(Mutex::new(client_type.into())),
            interceptor: None,
        }
    }
    
    /// Create a new `FlexibleClient` wrapping the given client
    #[must_use]
    pub fn new(client: Box<dyn LowLevelClient>) -> Self {
        Self { 
            inner: Arc::new(Mutex::new(client)),
            interceptor: None,
        }
    }
    
    /// Create a new `FlexibleClient` with an interceptor
    #[must_use]
    pub fn with_interceptor(&self, interceptor: Arc<dyn Interceptor>) -> Self {
        Self {
            inner: self.inner.clone(),
            interceptor: Some(interceptor),
        }
    }
    
       /// Create a new `FlexibleClient` with an interceptor
       #[must_use]
       pub fn with_file_interceptor(&self, path: PathBuf) -> Self {
        Self {
            inner: self.inner.clone(),
            interceptor: Some(Arc::new(FileInterceptor::new(path))),
        }
    }
    /// Create a `FlexibleClient` with a Claude client (explicit config)
    #[must_use]
    pub fn claude_with(config: ClaudeConfig) -> Self {
        use super::claude::ClaudeClient;
        Self::new(Box::new(ClaudeClient::new(config)))
    }

    /// Create a `FlexibleClient` with a Claude client using default config from env
    #[must_use]
    pub fn claude() -> Self {
        use super::claude::ClaudeClient;
        Self::new(Box::new(ClaudeClient::default()))
    }
    
    /// Create a `FlexibleClient` with a `DeepSeek` client (explicit config)
    #[must_use]
    pub fn deepseek_with(config: DeepSeekConfig) -> Self {
        use super::deepseek::DeepSeekClient;
        Self::new(Box::new(DeepSeekClient::new(config)))
    }

    /// Create a `FlexibleClient` with a `DeepSeek` client using default config from env
    #[must_use]
    pub fn deepseek() -> Self {
        use super::deepseek::DeepSeekClient;
        Self::new(Box::new(DeepSeekClient::default()))
    }

    /// Create a `FlexibleClient` with a ChatGPT-family client (OpenAI/Azure) based on env
    #[must_use]
    pub fn chatgpt() -> Self {
        // Reuse the same selection logic as in ClientType::ChatGPT
        Self::new(ClientType::ChatGPT.into())
    }

    /// Create a `FlexibleClient` by auto-selecting provider based on available env keys
    #[must_use]
    pub fn auto() -> Self {
        Self::new(ClientType::default().into())
    }
    
    
    /// Create a FlexibleClient with a mock and return the handle for configuration
    pub fn mock() -> (Self, Arc<super::mock::MockHandle>) {
        use super::mock::MockClient;
        let (mock_client, handle) = MockClient::new();
        let flexible = Self::new(Box::new(mock_client));
        (flexible, handle)
    }

    /// Create a FlexibleClient mock with predefined responses
    pub fn new_mock_with_responses(responses: Vec<super::mock::MockResponse>) -> (Self, Arc<super::mock::MockHandle>) {
        use super::mock::MockClient;
        let (mock_client, handle) = MockClient::with_responses(responses);
        let flexible = Self::new(Box::new(mock_client));
        (flexible, handle)
    }
    
    /// Convert into the inner boxed client (initializes if needed)
    pub fn into_inner(self) -> Result<Box<dyn LowLevelClient>, AIError> {
        let inner = self.inner.lock().unwrap().clone_box();
        Ok(inner)
    }

    /// Get a streaming reader for the raw model output.
    /// If the underlying client does not support true streaming, this will
    /// fallback to a one-shot response written into a duplex stream.
    pub fn stream_raw_reader(&self, prompt: String) -> Pin<Box<dyn AsyncRead + Send>> {
        // Try streaming first
        let client = {
            let inner = self.inner.lock().unwrap();
            inner.as_ref().clone_box()
        };
        if let Some(stream) = client.stream_raw(prompt.clone()) {
            // Map AIError to io::Error
            let io_stream = stream.map(|res| match res {
                Ok(bytes) => Ok::<Bytes, std::io::Error>(bytes),
                Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
            });
            let reader = StreamReader::new(io_stream);
            return Box::pin(reader);
        }

        // Fallback: one-shot ask_raw() written to a duplex
        let (mut tx, rx) = tokio::io::duplex(8 * 1024);
        tokio::spawn(async move {
            if let Ok(text) = client.ask_raw(prompt).await {
                use tokio::io::AsyncWriteExt;
                let _ = tx.write_all(text.as_bytes()).await;
            }
        });
        Box::pin(rx)
    }
}

impl Clone for FlexibleClient {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            interceptor: self.interceptor.clone(),
        }
    }
}

#[async_trait]
impl LowLevelClient for FlexibleClient {
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        
        // Clone the client to avoid holding the mutex across await
        let client = {
            let inner = self.inner.lock().unwrap();
            inner.as_ref().clone_box()
        };
        
        let response = client.ask_raw(prompt.clone()).await?;
        
        // Save to interceptor if present
        if let Some(interceptor) = &self.interceptor {
            if let Err(e) = interceptor.save(&prompt, &response).await {
                // Log error but don't fail the request
                eprintln!("Interceptor save failed: {}", e);
            }
        }
        
        Ok(response)
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }

    fn stream_raw(&self, prompt: String) -> Option<Pin<Box<dyn futures_core::stream::Stream<Item = Result<Bytes, AIError>> + Send>>> {
        // Delegate to underlying client's streaming capability
        let client = {
            let inner = self.inner.lock().unwrap();
            inner.as_ref().clone_box()
        };
        client.stream_raw(prompt)
    }
}
