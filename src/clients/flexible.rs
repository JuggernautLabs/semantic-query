use crate::clients::{ClaudeConfig, DeepSeekConfig};
use crate::core::{LowLevelClient};
use crate::error::{AIError};
use async_trait::async_trait;
use std::env;
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
        let client: Box<dyn LowLevelClient> = match self {
            ClientType::Claude => {
                use super::claude::ClaudeClient;
                Box::new(ClaudeClient::default())
            }
            ClientType::DeepSeek => {
                use super::deepseek::DeepSeekClient;
                Box::new(DeepSeekClient::default())
            }
            ClientType::Mock => {
                use super::mock::MockVoid;
                Box::new(MockVoid)
            }
        };
        client
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
}


impl FlexibleClient {
    /// Create a new FlexibleClient with lazy initialization
    pub fn new_lazy(client_type: ClientType) -> Self {
       
        Self { 
            inner: Arc::new(Mutex::new(client_type.into())),
        }
    }
    
    /// Create a new FlexibleClient wrapping the given client
    pub fn new(client: Box<dyn LowLevelClient>) -> Self {
        Self { 
            inner: Arc::new(Mutex::new(client)),
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
    
    /// Create a FlexibleClient with a mock client
    pub fn mock() -> Self {
        use super::mock::MockVoid;
        Self::new(Box::new(MockVoid))
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
        
        client.ask_raw(prompt).await
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}