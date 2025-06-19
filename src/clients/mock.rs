use async_trait::async_trait;

use crate::{core::LowLevelClient, error::AIError};


/// Mock client for testing that returns empty responses
#[derive(Debug, Clone, Default)]
pub struct MockVoid;

#[async_trait]
impl LowLevelClient for MockVoid {
    async fn ask_raw(&self, _prompt: String) -> Result<String, AIError> {
        Ok("{}".to_string())
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}