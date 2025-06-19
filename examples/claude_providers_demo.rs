use semantic_query::clients::claude::{ClaudeClient, ClaudeConfig, Provider, ClaudeModels, BedrockModels, VertexModels};
use semantic_query::core::LowLevelClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    println!("=== Claude Multi-Provider Demo ===\n");
    
    // Example 1: Anthropic API (default)
    println!("1. Creating Anthropic API client with Claude 4 Sonnet:");
    let anthropic_config = ClaudeConfig::anthropic(
        "your-api-key".to_string(),
        ClaudeModels::SONNET_4.to_string()
    );
    let anthropic_client = ClaudeClient::new(anthropic_config);
    println!("   Model: {}", ClaudeModels::SONNET_4);
    println!("   Provider: Anthropic\n");
    
    // Example 2: AWS Bedrock
    println!("2. Creating AWS Bedrock client with Claude 4 Opus:");
    let bedrock_config = ClaudeConfig::bedrock(
        "us-east-1".to_string(),
        BedrockModels::OPUS_4.to_string()
    );
    let bedrock_client = ClaudeClient::new(bedrock_config);
    println!("   Model: {}", BedrockModels::OPUS_4);
    println!("   Provider: AWS Bedrock\n");
    
    // Example 3: GCP Vertex AI
    println!("3. Creating GCP Vertex AI client with Claude 3.7 Sonnet:");
    let vertex_config = ClaudeConfig::vertex(
        "my-project".to_string(),
        "us-central1".to_string(),
        VertexModels::SONNET_3_7.to_string()
    );
    let vertex_client = ClaudeClient::new(vertex_config);
    println!("   Model: {}", VertexModels::SONNET_3_7);
    println!("   Provider: GCP Vertex AI\n");
    
    // Example 4: Using the default client
    println!("4. Creating default client (Anthropic with Haiku 3.5):");
    let default_client = ClaudeClient::default();
    println!("   Model: {} (default)", ClaudeModels::HAIKU_3_5);
    println!("   Provider: Anthropic (default)\n");
    
    // Show model mapping examples
    println!("=== Model Mapping Examples ===");
    println!("Anthropic API model names:");
    println!("  - Claude 4 Opus: {}", ClaudeModels::OPUS_4);
    println!("  - Claude 4 Sonnet: {}", ClaudeModels::SONNET_4);
    println!("  - Claude 3.7 Sonnet: {}", ClaudeModels::SONNET_3_7);
    println!("  - Claude 3.5 Haiku: {}", ClaudeModels::HAIKU_3_5);
    
    println!("\nAWS Bedrock model names:");
    println!("  - Claude 4 Opus: {}", BedrockModels::OPUS_4);
    println!("  - Claude 4 Sonnet: {}", BedrockModels::SONNET_4);
    println!("  - Claude 3.7 Sonnet: {}", BedrockModels::SONNET_3_7);
    println!("  - Claude 3.5 Haiku: {}", BedrockModels::HAIKU_3_5);
    
    println!("\nGCP Vertex AI model names:");
    println!("  - Claude 4 Opus: {}", VertexModels::OPUS_4);
    println!("  - Claude 4 Sonnet: {}", VertexModels::SONNET_4);
    println!("  - Claude 3.7 Sonnet: {}", VertexModels::SONNET_3_7);
    println!("  - Claude 3.5 Haiku: {}", VertexModels::HAIKU_3_5);
    
    println!("\n=== Configuration Examples ===");
    
    // Show automatic model name conversion
    let config_with_conversion = ClaudeConfig {
        provider: Provider::AwsBedrock,
        model: ClaudeModels::SONNET_4.to_string(), // Standard Anthropic name
        aws_region: Some("us-west-2".to_string()),
        ..Default::default()
    };
    println!("Standard model name: {}", config_with_conversion.model);
    println!("Converted for Bedrock: {}", config_with_conversion.get_model_for_provider());
    
    println!("\n✅ All configurations created successfully!");
    println!("Note: To actually make API calls, you would need to provide valid credentials:");
    println!("  - Anthropic: Set ANTHROPIC_API_KEY environment variable");
    println!("  - AWS Bedrock: Configure AWS credentials (access key, secret, region)");
    println!("  - GCP Vertex: Set up GCP service account credentials");
    
    Ok(())
}