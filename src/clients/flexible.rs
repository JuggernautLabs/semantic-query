use crate::core::{LowLevelClient};
use crate::error::{AIError};
use async_trait::async_trait;


/// Flexible client that wraps any LowLevelClient and provides factory functions
pub struct FlexibleClient {
    inner: Box<dyn LowLevelClient>,
}

impl std::fmt::Debug for FlexibleClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlexibleClient")
            .field("inner", &"<dyn LowLevelClient>")
            .finish()
    }
}

impl FlexibleClient {
    /// Create a new FlexibleClient wrapping the given client
    pub fn new(client: Box<dyn LowLevelClient>) -> Self {
        Self { inner: client }
    }
    
    /// Create a FlexibleClient with a Claude client
    pub fn claude() -> Self {
        use super::claude::ClaudeClient;
        Self::new(Box::new(ClaudeClient::default()))
    }
    
    /// Create a FlexibleClient with a DeepSeek client  
    pub fn deepseek() -> Self {
        use super::deepseek::DeepSeekClient;
        Self::new(Box::new(DeepSeekClient::default()))
    }
    
    /// Create a FlexibleClient with a mock client
    pub fn mock() -> Self {
        use super::mock::MockVoid;

        Self::new(Box::new(MockVoid::default()))
    }
    
    /// Get a reference to the inner client
    pub fn inner(&self) -> &Box<dyn LowLevelClient> {
        &self.inner
    }
    
    /// Convert into the inner boxed client
    pub fn into_inner(self) -> Box<dyn LowLevelClient> {
        self.inner
    }
}

impl Clone for FlexibleClient {
    fn clone(&self) -> Self {
        Self::new(self.inner.clone_box())
    }
}

#[async_trait]
impl LowLevelClient for FlexibleClient {
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        self.inner.ask_raw(prompt).await
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}