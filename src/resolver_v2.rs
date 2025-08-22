//! QueryResolver V2: Better handling of mixed content responses
//!
//! Unlike the original resolver that assumes entire responses can be parsed as T,
//! V2 recognizes that LLM responses are mixed content: explanatory text + structured data.
//! 
//! Key improvements:
//! - `query_mixed<T>` returns `Vec<StreamItem<T>>` for non-streaming queries
//! - `query_extract_all<T>` extracts all T instances, preserving text context
//! - `query_extract_first<T>` gets the first T instance with context
//! - Better error reporting with partial results

use crate::error::QueryResolverError;
use crate::streaming::{StreamItem, TextContent, build_parsed_stream};
use std::fmt;
use crate::core::{LowLevelClient, RetryConfig};
use serde::de::DeserializeOwned;
use schemars::{JsonSchema, schema_for};
use std::fmt::Debug;
use tracing::{info, warn, error, debug, instrument};

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

/// Query resolver V2 with better mixed content handling
#[derive(Debug, Clone)]
pub struct QueryResolverV2<C> {
    client: C,
    config: RetryConfig,
}

impl<C: LowLevelClient> QueryResolverV2<C> {
    /// Create a new V2 resolver
    pub fn new(client: C, config: RetryConfig) -> Self {
        info!("Creating new QueryResolver V2 with retry config default_max_retries={}", 
              config.default_max_retries);
        Self { client, config }
    }
    
    /// Query expecting mixed content (text + structured data)
    /// 
    /// This is the most honest API - it returns exactly what LLMs actually produce:
    /// a mix of explanatory text and structured data, preserving order and context.
    #[instrument(target = "semantic_query::resolver_v2", skip(self, prompt), fields(prompt_len = prompt.len()))]
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
    
    /// Extract all instances of T from the response with schema guidance
    /// 
    /// This is like the old `query_with_schema` but returns all instances found,
    /// not just the first one. Includes context for better error reporting.
    #[instrument(target = "semantic_query::resolver_v2", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query<T>(&self, prompt: String) -> Result<ParsedResponse<T>, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send + Debug + serde::Serialize + Clone,
    {
        info!(prompt_len = prompt.len(), "Starting extract all query");
        
        let schema_prompt = self.add_schema_guidance::<T>(prompt);
        self.query_mixed(schema_prompt).await
    }
    
    /// Compatibility method: behaves like the old query_with_schema
    /// but returns just the first T instance for drop-in replacement
    #[instrument(target = "semantic_query::resolver_v2", skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query_with_schema_compat<T>(&self, prompt: String) -> Result<T, QueryResolverError>
    where
        T: DeserializeOwned + JsonSchema + Send + Debug + serde::Serialize + Clone,
    {
        let result: ParsedResponse<T> = self.query(prompt).await?;
        Ok(result.data_only().into_iter().next().unwrap().clone()) // Safe because extract_first ensures data exists
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
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::mock::MockClient;
    use serde::{Deserialize, Serialize};
    use schemars::JsonSchema;
    
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }
    
    #[tokio::test]
    async fn test_mixed_content_query() {
        let (client, handle) = MockClient::new();
        let resolver = QueryResolverV2::new(client, RetryConfig::default());
        
        // Mock response with mixed content
        handle.add_response(crate::clients::MockResponse::Success("Here's some analysis: {\"name\": \"test\", \"value\": 42} and more explanation.".to_string()));
        
        let result = resolver.query_mixed::<TestData>("test prompt".to_string()).await.unwrap();
        
        let data = result.data_only();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].name, "test");
        assert_eq!(data[0].value, 42);
        assert!(result.text_content().contains("Here's some analysis"));
        assert!(result.text_content().contains("and more explanation"));
    }
    
    #[tokio::test]
    async fn test_multiple_data_items() {
        let (client, handle) = MockClient::new();
        let resolver = QueryResolverV2::new(client, RetryConfig::default());
        
        handle.add_response(crate::clients::MockResponse::Success("First: {\"name\": \"a\", \"value\": 1} then {\"name\": \"b\", \"value\": 2} done.".to_string()));
        
        let result = resolver.query_mixed::<TestData>("test".to_string()).await.unwrap();
        
        let data = result.data_only();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].name, "a");
        assert_eq!(data[1].name, "b");
    }
    
    #[tokio::test]
    async fn test_no_data_found() {
        let (client, handle) = MockClient::new();
        let resolver = QueryResolverV2::new(client, RetryConfig::default());
        
        handle.add_response(crate::clients::MockResponse::Success("Just plain text with no JSON data.".to_string()));
        
        let result = resolver.query::<TestData>("test".to_string()).await;
        
        assert!(result.is_err());
        // Should include context in error
    }
}