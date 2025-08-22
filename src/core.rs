//! Core querying API: wraps a low-level model client with resilient JSON extraction,
//! schema-aware prompting, and streaming responses.
//!
//! Quick start:
//! - **Recommended**: Use `QueryResolver::query<T>()` for schema-guided queries with mixed content
//! - **Advanced**: Use `QueryResolver::query_mixed<T>()` for raw mixed content without schema  
//! - **Streaming**: Use `QueryResolver::stream_query<T>()` for real-time token streaming
//! - **Legacy methods** (`query_deserialized`, `query_with_schema`) are deprecated stubs

use crate::error::{QueryResolverError, AIError, DataExtractionError};
use crate::streaming::{StreamItem, TextContent, build_parsed_stream};
use std::fmt;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::fmt::Debug;
use std::pin::Pin;
use async_trait::async_trait;
use tracing::{info, warn, debug, instrument};
use schemars::{JsonSchema, schema_for};
use futures_core::Stream;
use bytes::Bytes;

/// Type alias for raw byte streams from AI providers
pub type RawByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, AIError>> + Send>>;

/// Type alias for parsed streaming results
pub type ParsedStreamResult<T> = Result<Pin<Box<dyn Stream<Item = Result<StreamItem<T>, QueryResolverError>> + Send>>, QueryResolverError>;

/// A single item in an LLM response - either structured data or explanatory text
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseItem<T> {
    /// Structured data that was successfully parsed from JSON
    /// Contains both the parsed data and the original JSON string
    Data { 
        /// The parsed structured data
        data: T, 
        /// The original JSON string that was parsed
        original_text: String 
    },
    /// Explanatory text content from the LLM
    Text(TextContent),
}

/// Complete LLM response with mixed content (text + structured data)
#[derive(Debug, Clone)]
pub struct ParsedResponse<T> {
    /// All items in order (text and data)
    pub items: Vec<ResponseItem<T>>,
}

impl<T: JsonSchema + serde::Serialize + Clone> ParsedResponse<T> {
    /// Get only the structured data items
    pub fn data_only(&self) -> Vec<&T> {
        self.items.iter().filter_map(|item| match item {
            ResponseItem::Data { data, .. } => Some(data),
            ResponseItem::Text(_) => None,
        }).collect()
    }
    
    /// Get the complete text content (includes text around parsed JSON)
    pub fn text_content(&self) -> String {
        let mut result = String::new();
        for item in &self.items {
            match item {
                ResponseItem::Text(text) => {
                    if !result.is_empty() { result.push(' '); }
                    result.push_str(&text.text);
                }
                ResponseItem::Data { original_text, .. } => {
                    if !result.is_empty() { result.push(' '); }
                    result.push_str(original_text);
                }
            }
        }
        result
    }
    
    /// Get the first data item if any exists
    pub fn first_data(&self) -> Option<&T> {
        self.data_only().into_iter().next()
    }
    
    /// Get the first data item if any exists (convenience method)
    pub fn first(&self) -> Option<&T> {
        self.first_data()
    }
    
    /// Get the first data item, returning an error if none exists
    /// This is a convenience method for clean error handling when migrating from single-item APIs
    pub fn first_required(&self) -> Result<T, DataExtractionError> 
    where 
        T: Clone,
    {
        self.first()
            .cloned()
            .ok_or(DataExtractionError::NoDataFound)
    }
    
    /// Check if any data was extracted
    pub fn has_data(&self) -> bool {
        self.data_only().len() > 0
    }
    
    /// Get count of data items found
    pub fn data_count(&self) -> usize {
        self.data_only().len()
    }
    
    /// Convert StreamItems to ResponseItems
    fn from_stream_items(stream_items: Vec<StreamItem<T>>) -> Self {
        let items = stream_items.into_iter().filter_map(|item| match item {
            StreamItem::Data(data) => {
                // Fallback: re-serialize the data since we don't have original text
                let original_text = serde_json::to_string(&data)
                    .unwrap_or_else(|_| "[serialization failed]".to_string());
                Some(ResponseItem::Data { data, original_text })
            },
            StreamItem::Text(text) => Some(ResponseItem::Text(text)),
            StreamItem::Token(_) => None, // Tokens not relevant for non-streaming
        }).collect();
        
        Self { items }
    }
}

impl<T: fmt::Display> fmt::Display for ParsedResponse<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, item) in self.items.iter().enumerate() {
            if i > 0 { writeln!(f)?; }
            match item {
                ResponseItem::Text(text) => write!(f, "[Text] {}", text.text)?,
                ResponseItem::Data { data, original_text } => {
                    write!(f, "[Data] {} (original: {})", data, original_text)?
                },
            }
        }
        Ok(())
    }
}

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
        info!(default_max_retries = config.default_max_retries, "Creating new QueryResolver");
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

    /// Query expecting mixed content (text + structured data)
    /// 
    /// This is the main API - it returns exactly what LLMs actually produce:
    /// a mix of explanatory text and structured data, preserving order and context.
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query_mixed<T>(&self, prompt: String) -> Result<ParsedResponse<T>, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send + Debug + serde::Serialize + Clone,
    {
        info!(prompt_len = prompt.len(), "Starting mixed content query");
        
        let raw_response = self.client.ask_raw(prompt).await?;
        let stream_items = build_parsed_stream::<T>(&raw_response);
        let response = ParsedResponse::from_stream_items(stream_items);
        
        info!(data_count = response.data_count(), text_length = response.text_content().len(), 
              "Mixed content query completed");
              
        Ok(response)
    }
    
    /// Query with automatic JSON Schema guidance - the main recommended method
    /// 
    /// Automatically adds schema guidance and returns mixed content with context preserved.
    #[instrument(target = "semantic_query::resolver", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query<T>(&self, prompt: String) -> Result<ParsedResponse<T>, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send + Debug + serde::Serialize + Clone,
    {
        info!(prompt_len = prompt.len(), "Starting query");
        
        let schema_prompt = self.add_schema_guidance::<T>(prompt);
        self.query_mixed(schema_prompt).await
    }
    
    /// Add JSON schema guidance to a prompt
    fn add_schema_guidance<T>(&self, prompt: String) -> String
    where
        T: JsonSchema,
    {
        let schema = schema_for!(T);
        let schema_json = serde_json::to_string_pretty(&schema)
            .unwrap_or_else(|_| "Schema serialization failed".to_string());
            
        format!(
            "{}\n\n## Response Format\nPlease include valid JSON matching this schema somewhere in your response:\n```json\n{}\n```",
            prompt, schema_json
        )
    }
    
    // =============================================================================
    // DEPRECATED METHODS - Stubs only, use query() and query_mixed() instead
    // =============================================================================
    
    /// Return a deserialized value `T` using retry + stream-based JSON extraction.
    ///
    /// # Deprecated
    /// This method is deprecated and returns a stub error. 
    /// Use `query<T>().first()` for similar functionality with better mixed content handling.
    #[deprecated(since = "0.2.0", note = "Use query<T>().first() instead")]
    pub async fn query_deserialized<T>(&self, _prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + Send,
    {
        Err(QueryResolverError::JsonDeserialization(
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::Other, 
                "query_deserialized is deprecated - use query<T>().first() instead"
            )),
            "deprecated method called".to_string(),
        ))
    }
    
    /// Return a deserialized value `T` with automatic JSON Schema guidance.
    ///
    /// # Deprecated  
    /// This method is deprecated and returns a stub error.
    /// Use `query<T>().first()` for the same functionality with better context preservation.
    #[deprecated(since = "0.2.0", note = "Use query<T>().first() instead")]
    pub async fn query_with_schema<T>(&self, _prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send,
    {
        Err(QueryResolverError::JsonDeserialization(
            serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::Other, 
                "query_with_schema is deprecated - use query<T>().first() instead"
            )),
            "deprecated method called".to_string(),
        ))
    }

    /// Generate a JSON schema for the return type and append it to the prompt
    ///
    /// # Deprecated
    /// This method is deprecated and returns a stub.
    /// Schema guidance is now handled automatically by `query<T>()`.
    #[deprecated(since = "0.2.0", note = "Schema guidance is automatic in query<T>()")]
    pub fn augment_prompt_with_schema<T>(&self, prompt: String) -> String
    where
        T: JsonSchema,
    {
        format!("{} [DEPRECATED: use query<T>() instead]", prompt)
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
        
        // For streaming, we add schema guidance to help the model generate proper JSON
        let augmented_prompt = self.add_schema_guidance::<T>(prompt);
        debug!(prompt_len = augmented_prompt.len(), "Using schema-augmented prompt for streaming");
        
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
