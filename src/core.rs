//! Core querying API: wraps a low-level model client with resilient JSON extraction,
//! schema-aware prompting, and semantic streaming.
//!
//! Quick start:
//! - `query_deserialized<T>`: returns `T` from prompts where the model embeds JSON.
//! - `query_with_schema<T>`: appends JSON Schema for `T` to improve reliability.
//! - `query_semantic<T>`: returns `Vec<SemanticItem<T>>` preserving interleaved text + data.
//! - `query_semantic_stream<T, R: AsyncRead>`: emits `SemanticItem<T>` as the stream arrives.

use crate::error::{QueryResolverError, AIError};
use crate::json_utils;
use crate::semantic::SemanticItem;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::pin::Pin;
use async_trait::async_trait;
use tracing::{info, warn, error, debug, instrument};
use schemars::{JsonSchema, schema_for};
use futures_core::Stream;
use bytes::Bytes;

/// Low-level model client abstraction.
///
/// Implementors provide `ask_raw`, which executes a prompt and returns the raw
/// model text. Higher-level parsing and schema handling is performed by
/// `QueryResolver` using stream-first JSON extraction.
#[async_trait]
pub trait LowLevelClient: Send + Sync + Debug{
    /// The only method that implementations must provide
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError>;
    
    /// Clone this client into a boxed trait object
    fn clone_box(&self) -> Box<dyn LowLevelClient>;

    /// Optional: provide a streaming raw response as chunks of bytes.
    /// Default is None; providers can override to implement true streaming.
    fn stream_raw(&self, _prompt: String) -> Option<Pin<Box<dyn Stream<Item = Result<Bytes, AIError>> + Send>>> { None }
}

// Implement Clone for Box<dyn LowLevelClient>
impl Clone for Box<dyn LowLevelClient> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

// Implement LowLevelClient for Box<dyn LowLevelClient>
#[async_trait]
impl LowLevelClient for Box<dyn LowLevelClient> {
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError> {
        self.as_ref().ask_raw(prompt).await
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        self.as_ref().clone_box()
    }

    fn stream_raw(&self, prompt: String) -> Option<Pin<Box<dyn Stream<Item = Result<Bytes, AIError>> + Send>>> {
        self.as_ref().stream_raw(prompt)
    }
}



#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: HashMap<String, usize>,
    pub default_max_retries: usize,
}

impl Default for RetryConfig {
    fn default() -> Self {
        let mut max_retries = HashMap::new();
        max_retries.insert("rate_limit".to_string(), 1);
        max_retries.insert("api_error".to_string(), 1);
        max_retries.insert("http_error".to_string(), 1);
        max_retries.insert("json_parse_error".to_string(), 2);
        
        Self {
            max_retries,
            default_max_retries: 1,
        }
    }
}


#[derive(Clone)]
/// Query resolver that wraps a LowLevelClient and provides all generic methods.
/// This allows for flexible composition - you can have arrays of dyn LowLevelClient
/// and wrap them in QueryResolver as needed.
pub struct QueryResolver<C: LowLevelClient> {
    client: C,
    config: RetryConfig,
}

impl<C: LowLevelClient> QueryResolver<C> {
    pub fn new(client: C, config: RetryConfig) -> Self {
        info!(default_max_retries = config.default_max_retries, "Creating new QueryResolver with retry config");
        Self { client, config }
    }
    
    /// Get a reference to the underlying client
    pub fn client(&self) -> &C {
        &self.client
    }
    
    /// Get a reference to the retry configuration
    pub fn config(&self) -> &RetryConfig {
        &self.config
    }
    
    /// Update the retry configuration
    pub fn with_config(mut self, config: RetryConfig) -> Self {
        self.config = config;
        self
    }
    
    /// Return a deserialized value `T` using retry + stream-based JSON extraction.
    ///
    /// Usage:
    /// - For free-form prompts where the model emits JSON somewhere in the text.
    /// - Prefer `query_with_schema` when you can provide a schema for `T`.
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query_deserialized<T>(&self, prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + Send,
    {
        info!(prompt_len = prompt.len(), "Starting query");
        let result = self.ask_with_retry(prompt).await;
        match &result {
            Ok(_) => info!("Query completed successfully"),
            Err(e) => error!(error = %e, "Query failed"),
        }
        result
    }
    
    /// Return a deserialized value `T` with automatic JSON Schema guidance.
    ///
    /// Appends the JSON Schema for `T` to the prompt and enforces
    /// “JSON only” output, improving reliability.
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query_with_schema<T>(&self, prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send,
    {
        info!(prompt_len = prompt.len(), "Starting schema-aware query");
        let result = self.ask_with_schema(prompt).await;
        match &result {
            Ok(_) => info!("Schema-aware query completed successfully"),
            Err(e) => error!(error = %e, "Schema-aware query failed"),
        }
        result
    }

    /// Return a mixed, ordered vector of text and structured items.
    ///
    /// Returns an ordered vector where each element is either free-form text
    /// or a structured `T`, allowing downstream systems to retain full context
    /// and interleave commentary and data as the model deems useful.
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query_semantic<T>(&self, prompt: String) -> Result<Vec<SemanticItem<T>>, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send,
    {
        info!(prompt_len = prompt.len(), "Starting semantic query (text + data)");
        // Guide the model to output interleaved content using the schema for SemanticItem<T>
        let augmented = self.augment_prompt_with_semantic_schema::<T>(prompt);
        debug!(augmented_prompt_len = augmented.len(), "Generated semantic schema-augmented prompt");
        let raw = self.client.ask_raw(augmented).await?;
        let stream = crate::semantic::build_semantic_stream::<T>(&raw);
        Ok(stream)
    }
    
    /// Internal method for retry logic with JSON parsing
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    async fn ask_with_retry<T>(&self, prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + Send,
    {
        let mut attempt = 0;
        let mut context = String::new();
        
        info!(attempt = 0, max_retries = self.config.default_max_retries, "Starting retry loop for prompt");
        
        loop {
            let full_prompt = if context.is_empty() {
                prompt.clone()
            } else {
                format!("{}\n\nPrevious attempt failed: {}\nPlease fix the issue and respond with valid JSON.", prompt, context)
            };
            
            debug!(attempt = attempt + 1, prompt_len = full_prompt.len(), "Making API call");
            match self.client.ask_raw(full_prompt.clone()).await {
                Ok(raw) => {
                    debug!(response_len = raw.len(), "Received API response");
                    let items: Vec<crate::json_utils::ParsedOrUnknown<T>> = json_utils::deserialize_stream_map::<T>(&raw);
                    // Return first successfully parsed item
                    if let Some(parsed) = items.into_iter().find_map(|it| match it { json_utils::ParsedOrUnknown::Parsed(v) => Some(v), _ => None }) {
                        info!(attempt = attempt + 1, "Successfully parsed structured item from stream");
                        return Ok(parsed);
                    }

                    // No parsed item found
                    let max_retries = self.config.max_retries.get("json_parse_error")
                        .unwrap_or(&self.config.default_max_retries);

                    if attempt >= *max_retries {
                        error!(attempt = attempt + 1, max_retries = max_retries, "Max retries exceeded; no matching structures found");
                        return Err(QueryResolverError::JsonDeserialization(
                            serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::Other, "No matching JSON structure found in stream")),
                            raw,
                        ));
                    }

                    warn!(attempt = attempt + 1, max_retries = max_retries, "Retrying due to no matching structures in stream");
                    context = "No matching JSON structure found".to_string();
                    attempt += 1;
                }
                Err(ai_error) => {
                    warn!(error = %ai_error, attempt = attempt + 1, "API call failed");
                    let error_type = match &ai_error {
                        AIError::Claude(claude_err) => match claude_err {
                            crate::error::ClaudeError::RateLimit => "rate_limit",
                            crate::error::ClaudeError::Http(_) => "http_error",
                            crate::error::ClaudeError::Api(_) => "api_error",
                            _ => "other",
                        },
                        AIError::OpenAI(openai_err) => match openai_err {
                            crate::error::OpenAIError::RateLimit => "rate_limit",
                            crate::error::OpenAIError::Http(_) => "http_error", 
                            crate::error::OpenAIError::Api(_) => "api_error",
                            _ => "other",
                        },
                        AIError::DeepSeek(deepseek_err) => match deepseek_err {
                            crate::error::DeepSeekError::RateLimit => "rate_limit",
                            crate::error::DeepSeekError::Http(_) => "http_error",
                            crate::error::DeepSeekError::Api(_) => "api_error",
                            _ => "other",
                        },
                        AIError::Mock(_) => "mock_error",
                    };
                    
                    let max_retries = self.config.max_retries.get(error_type)
                        .unwrap_or(&self.config.default_max_retries);
                    
                    if attempt >= *max_retries {
                        error!(
                            error = %ai_error, 
                            error_type = error_type,
                            max_retries = max_retries,
                            "Max retries exceeded for API error"
                        );
                        return Err(QueryResolverError::Ai(ai_error));
                    }
                    
                    info!(
                        error_type = error_type,
                        attempt = attempt + 1,
                        max_retries = max_retries,
                        "Retrying after API error"
                    );
                    context = format!("API call failed: {}", ai_error);
                    attempt += 1;
                }
            }
        }
    }
    
    /// Generate a JSON schema for the return type and append it to the prompt
    pub fn augment_prompt_with_schema<T>(&self, prompt: String) -> String
    where
        T: JsonSchema,
    {
        let schema = schema_for!(T);
        let schema_json = serde_json::to_string_pretty(&schema)
            .unwrap_or_else(|_| "{}".to_string());
        
        debug!(schema_len = schema_json.len(), "Generated JSON schema for return type");
        
        format!(
            r#"{prompt}
You must output only a single JSON value that strictly conforms to the following JSON Schema. Do not include explanations, prose, code fences labels, or additional text — only the JSON value itself.
```json
{schema_json}
```
"#
        )
    }

    /// Augment prompt specifically for semantic streams (text + data items)
    pub fn augment_prompt_with_semantic_schema<T>(&self, prompt: String) -> String
    where
        T: JsonSchema,
    {
        let schema = schema_for!(Vec<crate::semantic::SemanticItem<T>>);
        let schema_json = serde_json::to_string_pretty(&schema)
            .unwrap_or_else(|_| "{}".to_string());

        debug!(schema_len = schema_json.len(), "Generated JSON schema for semantic stream");

        format!(
            r#"{prompt}
Return a single JSON array of items in order. Each item must be one of:
- Text: {{"kind":"Text","content":{{"text":"..."}}}}
- Data: {{"kind":"Data","content": <object matching the provided schema>}}
Do not include any text outside the JSON array. No code fences.
```json
{schema_json}
```
"#
        )
    }

    /// Ask with automatic schema-aware prompt augmentation
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
    async fn ask_with_schema<T>(&self, prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send,
    {
        info!("Starting schema-aware query");
        let augmented_prompt = self.augment_prompt_with_schema::<T>(prompt);
        debug!(augmented_prompt_len = augmented_prompt.len(), "Generated schema-augmented prompt");
        self.ask_with_retry(augmented_prompt).await
    }

    /// Stream `SemanticItem<T>` from any `AsyncRead` of model output.
    ///
    /// Wraps the stream-first JSON structure parser to emit `Text` chunks for
    /// non-JSON and `Data(T)` when a structure deserializes to `T`.
    ///
    /// Example (synthetic stream):
    /// ```no_run
    /// use tokio::io::{duplex, AsyncWriteExt};
    /// use futures_core::StreamExt;
    /// use semantic_query::core::{QueryResolver, RetryConfig};
    /// use semantic_query::semantic::{SemanticItem};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize)]
    /// struct Finding { message: String }
    ///
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let (mut tx, rx) = duplex(1024);
    /// tokio::spawn(async move {
    ///     let _ = tx.write_all(b"hello ").await;
    ///     let _ = tx.write_all(br#"{"message":"world"}"#).await;
    /// });
    /// let resolver = QueryResolver::new(semantic_query::clients::mock::MockVoid, RetryConfig::default());
    /// let mut stream = resolver.query_semantic_stream::<Finding,_>(rx, 1024);
    /// while let Some(item) = stream.next().await {
    ///     match item { SemanticItem::Text(t) => println!("text: {}", t.text), SemanticItem::Data(d) => println!("data: {}", d.message), }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn query_semantic_stream<T, R>(&self, reader: R, buf_size: usize) -> impl futures_core::stream::Stream<Item = SemanticItem<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + 'static,
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        crate::semantic::stream_semantic_from_async_read::<R, T>(reader, buf_size)
    }
}
