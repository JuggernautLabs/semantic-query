use crate::error::{QueryResolverError, AIError};
use crate::json_utils;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use async_trait::async_trait;
use tracing::{info, warn, error, debug, instrument};
use schemars::{JsonSchema, schema_for};

/// Low-level client trait that only requires implementing ask_raw.
/// This trait can be used as dyn LowLevelClient for dynamic dispatch.
/// JSON processing is handled by utility functions with a convenience method.
#[async_trait]
pub trait LowLevelClient: Send + Sync {

    /// The only method that implementations must provide
    async fn ask_raw(&self, prompt: String) -> Result<String, AIError>;
    
    /// Simple JSON extraction from a prompt response (default implementation)
    async fn ask_json(&self, prompt: String) -> Result<String, AIError> {
        let raw_response = self.ask_raw(prompt).await?;
        Ok(json_utils::find_json(&raw_response))
    }
    
    /// Clone this client into a boxed trait object
    fn clone_box(&self) -> Box<dyn LowLevelClient>;
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
    
    /// Query with retry logic and automatic JSON parsing
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
    pub async fn query<T>(&self, prompt: String) -> Result<T, QueryResolverError>
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
    
    /// Query with automatic schema-aware prompt augmentation
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
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
    
    /// Internal method for retry logic with JSON parsing
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
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
            match self.client.ask_json(full_prompt.clone()).await {
                Ok(response) => {
                    debug!(response_len = response.len(), "Received API response");
                    match serde_json::from_str::<T>(&response) {
                        Ok(parsed) => {
                            info!(attempt = attempt + 1, "Successfully parsed JSON response");
                            return Ok(parsed);
                        },
                        Err(json_err) => {
                            warn!(
                                error = %json_err, 
                                response_preview = &response[..response.len().min(200)],
                                "Initial JSON parsing failed, trying advanced extraction"
                            );
                            
                            // Try advanced JSON extraction on the raw response
                            if let Ok(raw_response) = self.client.ask_raw(full_prompt.clone()).await {
                                if let Some(extracted_json) = json_utils::extract_json_advanced(&raw_response) {
                                    debug!(extracted_len = extracted_json.len(), "Trying to parse extracted JSON after initial failure");
                                    match serde_json::from_str::<T>(&extracted_json) {
                                        Ok(parsed) => {
                                            info!(attempt = attempt + 1, "Successfully parsed extracted JSON after initial deserialization failure");
                                            return Ok(parsed);
                                        },
                                        Err(extracted_err) => {
                                            warn!(
                                                error = %extracted_err,
                                                extracted_preview = &extracted_json[..extracted_json.len().min(200)],
                                                "Advanced extraction also failed to parse"
                                            );
                                        }
                                    }
                                } else {
                                    warn!("Advanced extraction could not find valid JSON in raw response");
                                }
                            }
                            
                            // If we're at max retries, return the error
                            let max_retries = self.config.max_retries.get("json_parse_error")
                                .unwrap_or(&self.config.default_max_retries);
                            
                            if attempt >= *max_retries {
                                error!(
                                    error = %json_err,
                                    attempt = attempt + 1,
                                    max_retries = max_retries,
                                    "Max retries exceeded for JSON parsing"
                                );
                                return Err(QueryResolverError::JsonDeserialization(json_err, response));
                            }
                            
                            // Otherwise, retry with context about the JSON parsing failure
                            warn!(
                                attempt = attempt + 1,
                                max_retries = max_retries,
                                "Retrying due to JSON parsing failure"
                            );
                            context = format!("JSON parsing failed: {}. Response was: {}", 
                                             json_err, 
                                             &response[..response.len().min(500)]);
                            attempt += 1;
                        }
                    }
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

Please respond with JSON that matches this exact schema:

```json
{schema_json}
```

Your response must be valid JSON that can be parsed into this structure. Include all required fields and follow the specified types."#
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
}


