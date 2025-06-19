
use std::env;

use semantic_query::clients::*;
use semantic_query::core::*;
/// Supported AI clients for testing
#[derive(Debug, Clone)]
pub enum ClientType {
    Claude,
    DeepSeek, 
    Mock,
}

impl ClientType {
    /// Parse client type from string (case insensitive)
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "deepseek" => Ok(Self::DeepSeek),
            "mock" => Ok(Self::Mock),
            _ => Err(format!("Unknown client type: '{}'. Supported: claude, deepseek, mock", s))
        }
    }
    
    /// Get the default client type based on available API keys
    pub fn default() -> Self {
        // Check for API keys in order of preference
        if env::var("ANTHROPIC_API_KEY").is_ok() || 
           std::fs::read_to_string(".env").map_or(false, |content| content.contains("ANTHROPIC_API_KEY")) {
            Self::Claude
        } else if env::var("DEEPSEEK_API_KEY").is_ok() || 
                 std::fs::read_to_string(".env").map_or(false, |content| content.contains("DEEPSEEK_API_KEY")) {
            Self::DeepSeek
        } else {
            Self::Mock
        }
    }
}


/// Get the configured client type from environment
pub fn get_client_type() -> ClientType {
    env::var("TEST_CLIENT")
        .ok()
        .and_then(|s| ClientType::from_str(&s).ok())
        .unwrap_or_else(ClientType::default)
}

/// Create a FlexibleClient based on the client type
fn create_flexible_client(client_type: ClientType) -> FlexibleClient {
    match client_type {
        ClientType::Claude => FlexibleClient::claude(),
        ClientType::DeepSeek => FlexibleClient::deepseek(), 
        ClientType::Mock => FlexibleClient::mock(),
    }
}



/// Create a new owned FlexibleClient instance using the configured client type
pub fn create_default_flexible_client() -> FlexibleClient {
    create_flexible_client(get_client_type())
}


/// Create a new owned instance using the configured client type
pub fn create_default_boxed_client() -> Box<dyn LowLevelClient> {
    create_default_flexible_client().into_inner()
}

/// Create a QueryResolver with the configured test client using Box<dyn LowLevelClient>
pub fn create_test_resolver() -> QueryResolver<Box<dyn LowLevelClient>> {
    let client = create_default_boxed_client();
    QueryResolver::new(client, RetryConfig::default())
}

/// Create a QueryResolver with custom retry configuration using Box<dyn LowLevelClient>
pub fn create_test_resolver_with_config(config: RetryConfig) -> QueryResolver<Box<dyn LowLevelClient>> {
    let client = create_default_boxed_client();
    QueryResolver::new(client, config)
}


/// Check if we should skip integration tests (i.e., we're using MockVoid)
pub fn should_skip_integration_tests() -> bool {
    matches!(get_client_type(), ClientType::Mock)
}

/// Print test client information
pub fn print_test_client_info() {
    let client_type = get_client_type();
    println!("🧪 Test Configuration:");
    println!("   Client: {:?}", client_type);
    
    match client_type {
        ClientType::Claude => {
            if env::var("ANTHROPIC_API_KEY").is_ok() {
                println!("   API Key: ✅ Found in environment");
            } else {
                println!("   API Key: ✅ Found in .env file");
            }
        },
        ClientType::DeepSeek => {
            if env::var("DEEPSEEK_API_KEY").is_ok() {
                println!("   API Key: ✅ Found in environment");
            } else {
                println!("   API Key: ✅ Found in .env file");
            }
        },
        ClientType::Mock => {
            println!("   Mode: Mock (no API calls will be made)");
        }
    }
    
    println!("   Override with: TEST_CLIENT=claude|deepseek|mock");
    println!();
}
