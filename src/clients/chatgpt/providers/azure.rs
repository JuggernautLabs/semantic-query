use crate::core::LowLevelClient;
use crate::clients::chatgpt::models::OpenAIModel;
use crate::error::{AIError, OpenAIError};
use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::{StreamExt, TryStreamExt};
use serde::Deserialize;
use tracing::instrument;

/// Azure OpenAI client (ChatGPT family) with streaming support.
#[derive(Debug, Clone)]
pub struct AzureOpenAIConfig {
    pub endpoint: String,                 // e.g., https://my-azure.openai.azure.com
    pub api_key: String,                  // AZURE_OPENAI_API_KEY
    pub deployment: String,               // model deployment name
    pub api_version: String,              // e.g., 2024-06-01
    pub model: OpenAIModel,               // used only for logging
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for AzureOpenAIConfig {
    fn default() -> Self {
        Self {
            endpoint: std::env::var("AZURE_OPENAI_ENDPOINT").unwrap_or_default(),
            api_key: std::env::var("AZURE_OPENAI_API_KEY").unwrap_or_default(),
            deployment: std::env::var("AZURE_OPENAI_DEPLOYMENT").unwrap_or_else(|_| "gpt-4o-mini".into()),
            api_version: std::env::var("AZURE_OPENAI_API_VERSION").unwrap_or_else(|_| "2024-06-01".into()),
            model: OpenAIModel::Gpt4oMini,
            max_tokens: 1024,
            temperature: 0.2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AzureOpenAIClient {
    config: AzureOpenAIConfig,
    http: reqwest::Client,
}

impl AzureOpenAIClient {
    pub fn new(config: AzureOpenAIConfig) -> Self { Self { config, http: reqwest::Client::new() } }

    fn url(&self) -> String {
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.config.endpoint.trim_end_matches('/'),
            self.config.deployment,
            self.config.api_version
        )
    }

    fn body(&self, prompt: String, stream: bool) -> serde_json::Value {
        serde_json::json!({
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "stream": stream,
            "messages": [
                {"role": "user", "content": prompt}
            ]
        })
    }
}

#[async_trait]
impl LowLevelClient for AzureOpenAIClient {
    #[instrument(skip(self, prompt), fields(model = %self.config.model.id()))]
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        let resp = self.http
            .post(self.url())
            .header("api-key", &self.config.api_key)
            .json(&self.body(prompt, false))
            .send().await
            .map_err(|e| AIError::OpenAI(OpenAIError::Http(e.to_string())))?;

        if resp.status() == 401 { return Err(AIError::OpenAI(OpenAIError::Authentication)); }
        if resp.status() == 429 { return Err(AIError::OpenAI(OpenAIError::RateLimit)); }
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AIError::OpenAI(OpenAIError::Api(txt)));
        }

        #[derive(Deserialize)]
        struct Choices { choices: Vec<Choice> }
        #[derive(Deserialize)]
        struct Choice { message: Msg }
        #[derive(Deserialize)]
        struct Msg { content: String }

        let parsed: Choices = resp.json().await
            .map_err(|e| AIError::OpenAI(OpenAIError::Http(e.to_string())))?;
        let content = parsed.choices.first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| AIError::OpenAI(OpenAIError::Api("No choices".into())))?;
        Ok(content)
    }

    fn clone_box(&self) -> Box<dyn LowLevelClient> { Box::new(self.clone()) }

    fn stream_raw(&self, prompt: String) -> Option<std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, AIError>> + Send>>> {
        let req = self.http
            .post(self.url())
            .header("api-key", &self.config.api_key)
            .json(&self.body(prompt, true));
        let fut = async move {
            let resp = req.send().await.map_err(|e| AIError::OpenAI(OpenAIError::Http(e.to_string())))?;
            if resp.status() == 401 { return Err(AIError::OpenAI(OpenAIError::Authentication)); }
            if resp.status() == 429 { return Err(AIError::OpenAI(OpenAIError::RateLimit)); }
            if !resp.status().is_success() {
                let txt = resp.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                return Err(AIError::OpenAI(OpenAIError::Api(txt)));
            }
            Ok(resp.bytes_stream().map(|r| r.map_err(|e| AIError::OpenAI(OpenAIError::Http(e.to_string())))))
        };
        let s = async_stream::try_stream! {
            let mut bytes_stream = fut.await?;
            while let Some(chunk) = bytes_stream.next().await {
                let b = chunk?;
                yield b;
            }
        };
        Some(Box::pin(s.map_err(|e| e)))
    }
}

