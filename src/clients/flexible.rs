use crate::clients::{ClaudeConfig, DeepSeekConfig};
use crate::core::{LowLevelClient};
use crate::error::{AIError};
use crate::interceptors::{FileInterceptor, Interceptor};
use async_trait::async_trait;
use std::env;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};


/// Client type for lazy initialization
#[derive(Debug, Clone)]
pub enum ClientType {
    Claude,
    DeepSeek,
    Mock,
}

impl Into<Box<dyn LowLevelClient>> for ClientType {
    fn into(self) -> Box<dyn LowLevelClient> {
        match self {
            ClientType::Claude => {
                use super::claude::ClaudeClient;
                Box::new(ClaudeClient::default())
            }
            ClientType::DeepSeek => {
                use super::deepseek::DeepSeekClient;
                Box::new(DeepSeekClient::default())
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
        } else {
            Self::Mock
        }
    }
}
impl ClientType {
    /// Parse client type from string (case insensitive)
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "deepseek" => Ok(Self::DeepSeek),
            "mock" => Ok(Self::Mock),
            _ => Err(format!("Unknown client type: '{}'. Supported: claude, deepseek, mock", s))
        }
    }
    
    /// Create a mock variant that returns both the client type and a handle
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
            ClientType::Mock => write!(f, "Mock"),
        }
    }
}


#[derive(Debug)]
/// Flexible client that wraps any LowLevelClient and provides factory functions
pub struct FlexibleClient {
    inner: Arc<Mutex<Box<dyn LowLevelClient>>>,
    interceptor: Option<Arc<dyn Interceptor>>,
}


impl FlexibleClient {
    /// Create a new FlexibleClient with lazy initialization
    pub fn new_lazy(client_type: ClientType) -> Self {
       
        Self { 
            inner: Arc::new(Mutex::new(client_type.into())),
            interceptor: None,
        }
    }
    
    /// Create a new FlexibleClient wrapping the given client
    pub fn new(client: Box<dyn LowLevelClient>) -> Self {
        Self { 
            inner: Arc::new(Mutex::new(client)),
            interceptor: None,
        }
    }
    
    /// Create a new FlexibleClient with an interceptor
    pub fn with_interceptor(&self, interceptor: Arc<dyn Interceptor>) -> Self {
        Self {
            inner: self.inner.clone(),
            interceptor: Some(interceptor),
        }
    }
    
       /// Create a new FlexibleClient with an interceptor
       pub fn with_file_interceptor(&self, path: PathBuf) -> Self {
        Self {
            inner: self.inner.clone(),
            interceptor: Some(Arc::new(FileInterceptor::new(path))),
        }
    }
    /// Create a FlexibleClient with a Claude client
    pub fn claude(config: ClaudeConfig) -> Self {
        use super::claude::ClaudeClient;
        Self::new(Box::new(ClaudeClient::new(config)))
    }
    
    /// Create a FlexibleClient with a DeepSeek client  
    pub fn deepseek(config: DeepSeekConfig) -> Self {
        use super::deepseek::DeepSeekClient;
        Self::new(Box::new(DeepSeekClient::new(config)))
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
}