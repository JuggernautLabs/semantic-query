use semantic_query::clients::claude::{ClaudeClient, ClaudeConfig, Provider, ClaudeModel};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    println!("=== Claude Multi-Provider Demo ===\n");
    println!("This demo shows how to use the same model (e.g., Opus 4) across different providers\n");
    
    // Example 1: Same model across different providers
    println!("🚀 Using Claude 4 Opus across different providers:");
    
    // Anthropic API
    println!("1. Anthropic API:");
    let anthropic_config = ClaudeConfig::anthropic(
        "your-anthropic-api-key".to_string(),
        ClaudeModel::Opus4
    );
    let anthropic_client = ClaudeClient::new(anthropic_config.clone());
    println!("   Model: {} ({})", ClaudeModel::Opus4.display_name(), anthropic_config.get_model_for_provider());
    println!("   Provider: Anthropic\n");
    
    // AWS Bedrock - Same model, different provider
    println!("2. AWS Bedrock:");
    let bedrock_config = ClaudeConfig::bedrock(
        "us-east-1".to_string(),
        ClaudeModel::Opus4  // Same model!
    );
    let bedrock_client = ClaudeClient::new(bedrock_config.clone());
    println!("   Model: {} ({})", ClaudeModel::Opus4.display_name(), bedrock_config.get_model_for_provider());
    println!("   Provider: AWS Bedrock\n");
    
    // GCP Vertex AI - Same model, different provider
    println!("3. GCP Vertex AI:");
    let vertex_config = ClaudeConfig::vertex(
        "my-project".to_string(),
        "us-central1".to_string(),
        ClaudeModel::Opus4  // Same model!
    );
    let vertex_client = ClaudeClient::new(vertex_config.clone());
    println!("   Model: {} ({})", ClaudeModel::Opus4.display_name(), vertex_config.get_model_for_provider());
    println!("   Provider: GCP Vertex AI\n");
    
    // Example 2: Easy provider switching
    println!("🔄 Easy provider switching with builder pattern:");
    let base_config = ClaudeConfig::new(Provider::Anthropic, ClaudeModel::Sonnet4);
    
    // Switch to Bedrock
    let bedrock_config = base_config.clone()
        .with_provider(Provider::AwsBedrock)
        .with_model(ClaudeModel::Sonnet4);  // Same model
    println!("   Switched to Bedrock: {}", bedrock_config.get_model_for_provider());
    
    // Switch to Vertex
    let vertex_config = base_config.clone()
        .with_provider(Provider::GcpVertex)
        .with_model(ClaudeModel::Sonnet4);  // Same model
    println!("   Switched to Vertex: {}", vertex_config.get_model_for_provider());
    
    // Example 3: All available models
    println!("\n📋 All available models:");
    let models = [
        ClaudeModel::Opus4,
        ClaudeModel::Sonnet4,
        ClaudeModel::Sonnet37,
        ClaudeModel::Haiku35,
        ClaudeModel::Sonnet35V2,
        ClaudeModel::Sonnet35,
        ClaudeModel::Opus3,
        ClaudeModel::Sonnet3,
        ClaudeModel::Haiku3,
    ];
    
    for model in &models {
        println!("   {} - Anthropic: {}, Bedrock: {}, Vertex: {}", 
            model.display_name(),
            model.anthropic_model_id(),
            model.bedrock_model_id(),
            model.vertex_model_id()
        );
    }
    
    // Example 4: Default configuration
    println!("\n🎯 Using default client:");
    let default_client = ClaudeClient::default();
    println!("   Default model: {} via Anthropic", ClaudeModel::default().display_name());
    
    println!("\n✅ Demo completed successfully!");
    println!("Key benefits:");
    println!("  • Abstract by model name, not platform-specific IDs");
    println!("  • Easy provider switching with same model");
    println!("  • Automatic model ID translation per provider");
    println!("  • Builder pattern for configuration flexibility");
    
    Ok(())
}