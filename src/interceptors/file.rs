use super::Interceptor;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use chrono::Utc;

#[derive(Debug)]
pub struct FileInterceptor {
    base_path: PathBuf,
}

impl FileInterceptor {
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }
}

#[async_trait]
impl Interceptor for FileInterceptor {
    async fn save(&self, prompt: &str, response: &str) -> Result<(), Box<dyn std::error::Error>> {
        let timestamp = Utc::now();
        let filename = format!("query_{}.md", timestamp.format("%Y%m%d_%H%M%S_%3f"));
        let file_path = self.base_path.join(filename);
        
        // Ensure the directory exists
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        
        let content = format!(
            "# Prompt\n\n{}\n\n# Response\n\n{}\n",
            prompt,
            response
        );
        
        let mut file = fs::File::create(&file_path).await?;
        file.write_all(content.as_bytes()).await?;
        file.flush().await?;
        
        Ok(())
    }
}