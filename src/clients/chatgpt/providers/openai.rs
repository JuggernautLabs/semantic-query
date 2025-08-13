use crate::core::LowLevelClient;
use crate::clients::chatgpt::models::OpenAIModel;
use crate::error::{AIError, OpenAIError};
use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::{StreamExt, TryStreamExt};
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub model: OpenAIModel,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            model: OpenAIModel::Gpt4oMini,
            max_tokens: 1024,
            temperature: 0.2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OpenAIClient {
    config: OpenAIConfig,
    http: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(config: OpenAIConfig) -> Self { Self { config, http: reqwest::Client::new() } }

    fn messages_body(&self, prompt: String) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model.id(),
            "max_tokens": self.config.max_tokens,
            "temperature": self.config.temperature,
            "messages": [
                {"role": "user", "content": prompt}
            ]
        })
    }
}

#[async_trait]
impl LowLevelClient for OpenAIClient {
    #[instrument(skip(self, prompt), fields(model = %self.config.model.id()))]
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        let body = self.messages_body(prompt);
        let resp = self.http
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.config.api_key)
            .json(&body)
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
        let body = {
            let mut v = self.messages_body(prompt);
            if let Some(obj) = v.as_object_mut() {
                obj.insert("stream".into(), serde_json::Value::Bool(true));
            }
            v
        };
        let req = self.http
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.config.api_key)
            .json(&body);
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

