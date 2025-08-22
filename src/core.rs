//! Core querying API: wraps a low-level model client with resilient JSON extraction,
//! schema-aware prompting, and streaming responses.
//!
//! # Deprecation Notice
//! The V1 QueryResolver methods (`query_deserialized`, `query_with_schema`) are deprecated.
//! Use `QueryResolverV2` from the `resolver_v2` module for better mixed content handling.
//!
//! Quick start:
//! - **Recommended**: Use `QueryResolverV2::query_extract_first<T>()` for single items
//! - **Recommended**: Use `QueryResolverV2::query_extract_all<T>()` for multiple items
//! - **Recommended**: Use `QueryResolverV2::query_mixed<T>()` for mixed content with context
//! - `query_stream<T, R: AsyncRead>`: emits `StreamItem<T>` as the stream arrives.

use crate::error::{QueryResolverError, AIError};
use crate::json_utils;
use crate::streaming::StreamItem;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::pin::Pin;
use async_trait::async_trait;
use tracing::{info, warn, error, debug, instrument};
use schemars::{JsonSchema, schema_for};
use futures_core::Stream;
use bytes::Bytes;

/// Type alias for raw byte streams from AI providers
pub type RawByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, AIError>> + Send>>;

/// Type alias for parsed streaming results
pub type ParsedStreamResult<T> = Result<Pin<Box<dyn Stream<Item = Result<StreamItem<T>, QueryResolverError>> + Send>>, QueryResolverError>;

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
    fn stream_raw(&self, _prompt: String) -> Option<RawByteStream> { None }
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

    fn stream_raw(&self, prompt: String) -> Option<RawByteStream> {
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
    /// 
    /// # Deprecated
    /// This method is deprecated. Use `QueryResolverV2::query_extract_first()` for better
    /// mixed content handling and error reporting. See `resolver_v2` module for migration.
    #[deprecated(since = "0.2.0", note = "Use QueryResolverV2::query_extract_first() instead")]
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query_deserialized<T>(&self, prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + Send,
    {
        info!(prompt_len = prompt.len(), "Starting query");
        // Simplified implementation for deprecated method - no retry logic
        match self.client.ask_raw(prompt).await {
            Ok(raw) => {
                debug!(response_len = raw.len(), "Received API response");
                let items: Vec<crate::json_utils::ParsedOrUnknown<T>> = json_utils::deserialize_stream_map::<T>(&raw);
                if let Some(parsed) = items.into_iter().find_map(|it| match it { 
                    json_utils::ParsedOrUnknown::Parsed(v) => Some(v), 
                    _ => None 
                }) {
                    info!("Successfully parsed structured item from stream");
                    Ok(parsed)
                } else {
                    error!("No matching JSON structure found in stream");
                    Err(QueryResolverError::JsonDeserialization(
                        serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::Other, "No matching JSON structure found in stream")),
                        raw,
                    ))
                }
            }
            Err(ai_error) => {
                error!(error = %ai_error, "API call failed");
                Err(QueryResolverError::Ai(ai_error))
            }
        }
    }
    
    /// Return a deserialized value `T` with automatic JSON Schema guidance.
    ///
    /// Appends the JSON Schema for `T` to the prompt and guides the model to
    /// include a valid JSON value somewhere in the response (our parser can
    /// extract JSON from interleaved text or streamed output).
    /// 
    /// # Deprecated
    /// This method is deprecated. Use `QueryResolverV2::query_extract_first()` for better
    /// mixed content handling that preserves context. For compatibility, use
    /// `QueryResolverV2::query_with_schema_compat()`. See `resolver_v2` module for migration.
    #[deprecated(since = "0.2.0", note = "Use QueryResolverV2::query_extract_first() instead")]
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
Include at least one JSON value that strictly conforms to the following JSON Schema. You may include additional explanatory text before or after; the JSON must be valid and can appear anywhere in your response.
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
        // Simplified implementation for deprecated method - no retry logic
        match self.client.ask_raw(augmented_prompt).await {
            Ok(raw) => {
                debug!(response_len = raw.len(), "Received API response");
                let items: Vec<crate::json_utils::ParsedOrUnknown<T>> = json_utils::deserialize_stream_map::<T>(&raw);
                if let Some(parsed) = items.into_iter().find_map(|it| match it { 
                    json_utils::ParsedOrUnknown::Parsed(v) => Some(v), 
                    _ => None 
                }) {
                    info!("Successfully parsed structured item from stream");
                    Ok(parsed)
                } else {
                    error!("No matching JSON structure found in stream");
                    Err(QueryResolverError::JsonDeserialization(
                        serde_json::Error::io(std::io::Error::new(std::io::ErrorKind::Other, "No matching JSON structure found in stream")),
                        raw,
                    ))
                }
            }
            Err(ai_error) => {
                error!(error = %ai_error, "API call failed");
                Err(QueryResolverError::Ai(ai_error))
            }
        }
    }

    /// Stream items (text + structured data) from a live model response.
    ///
    /// This is the high-level streaming API that handles all the complexity internally:
    /// - Automatically augments prompt with schema guidance for T (JSON can appear anywhere)
    /// - Initiates streaming API call  
    /// - Parses response into Text/Data(T) items in real-time
    /// - No manual buffer management or token handling required
    ///
    /// Example:
    /// ```no_run
    /// use futures_util::StreamExt;
    /// use semantic_query::core::{QueryResolver, RetryConfig};
    /// use semantic_query::streaming::{StreamItem};
    /// use semantic_query::clients::flexible::FlexibleClient;
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct ToolCall { name: String, args: serde_json::Value }
    ///
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = FlexibleClient::mock().0;
    /// let resolver = QueryResolver::new(client, RetryConfig::default());
    /// let mut stream = resolver.stream_query::<ToolCall>("Think step-by-step and use tools".to_string()).await?;
    /// while let Some(item) = stream.next().await {
    ///     match item {
    ///         Ok(StreamItem::Token(tok)) => print!("{}", tok), // Real-time tokens
    ///         Ok(StreamItem::Text(t)) => println!("[chat] {}", t.text),
    ///         Ok(StreamItem::Data(d)) => println!("[tool] {}", d.name),
    ///         Err(e) => eprintln!("Stream error: {}", e),
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn stream_query<T>(&self, prompt: String) -> ParsedStreamResult<T>
    where
        T: DeserializeOwned + JsonSchema + Send + 'static,
    {
        info!(prompt_len = prompt.len(), "Starting streaming query");
        
        // For streaming, we don't want the semantic schema - just the data schema
        // The stream items are created by our parser, not returned by the model
        let augmented_prompt = self.augment_prompt_with_schema::<T>(prompt);
        debug!(prompt_len = augmented_prompt.len(), "Using raw prompt for streaming");
        
        // Get streaming response
        let stream = self.client.stream_raw(augmented_prompt)
            .ok_or_else(|| {
                warn!("Client does not support streaming");
                crate::error::QueryResolverError::Ai(crate::error::AIError::Mock("Client does not support streaming".to_string()))
            })?;
        
        info!("Successfully initiated streaming response");
        
        // Convert SSE bytes stream to stream items and box it
        Ok(Box::pin(crate::streaming::stream_from_sse_bytes::<T>(stream)))
    }

    /// Stream `StreamItem<T>` from any `AsyncRead` of model output.
    ///
    /// Lower-level API for when you already have a reader. Most users should use
    /// `stream_query()` instead for the full end-to-end experience.
    ///
    /// Example (synthetic stream):
    /// ```no_run
    /// use tokio::io::{duplex, AsyncWriteExt};
    /// use futures_util::{StreamExt, pin_mut};
    /// use semantic_query::core::{QueryResolver, RetryConfig};
    /// use semantic_query::streaming::{StreamItem};
    /// use serde::Deserialize;
    /// use schemars::JsonSchema;
    ///
    /// #[derive(Deserialize, JsonSchema)]
    /// struct Finding { message: String }
    ///
    /// # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
    /// let (mut tx, rx) = duplex(1024);
    /// tokio::spawn(async move {
    ///     let _ = tx.write_all(b"hello ").await;
    ///     let _ = tx.write_all(br#"{"message":"world"}"#).await;
    /// });
    /// let resolver = QueryResolver::new(semantic_query::clients::mock::MockVoid, RetryConfig::default());
    /// let s = resolver.query_stream::<Finding,_>(rx, 1024);
    /// pin_mut!(s);
    /// while let Some(item) = s.next().await {
    ///     match item { StreamItem::Text(t) => println!("text: {}", t.text), StreamItem::Data(d) => println!("data: {}", d.message), }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn query_stream<T, R>(&self, reader: R, buf_size: usize) -> impl futures_core::stream::Stream<Item = StreamItem<T>>
    where
        T: DeserializeOwned + JsonSchema + Send + 'static,
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
    {
        crate::streaming::stream_from_async_read::<R, T>(reader, buf_size)
    }
}
