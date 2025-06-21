use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait Interceptor: Send + Sync + Debug {
    async fn save(&self, prompt: &str, response: &str) -> Result<(), Box<dyn std::error::Error>>;
}

pub mod file;
pub use file::FileInterceptor;