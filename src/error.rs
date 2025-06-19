use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueryResolverError {
    #[error("AI error: {0}")]
    Ai(#[from] AIError),
    #[error("JSON deserialization error: {0}. Raw response: {1}")]
    JsonDeserialization(#[source] serde_json::Error, String),
    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}

#[derive(Error, Debug)]
pub enum AIError {
    #[error("Claude API error: {0}")]
    Claude(#[from] ClaudeError),
    #[error("OpenAI API error: {0}")]
    OpenAI(#[from] OpenAIError),
    #[error("DeepSeek API error: {0}")]
    DeepSeek(#[from] DeepSeekError),
}

#[derive(Error, Debug)]
pub enum ClaudeError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Authentication failed")]
    Authentication,
}

#[derive(Error, Debug)]
pub enum OpenAIError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Authentication failed")]
    Authentication,
}

#[derive(Error, Debug)]
pub enum DeepSeekError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Authentication failed")]
    Authentication,
}