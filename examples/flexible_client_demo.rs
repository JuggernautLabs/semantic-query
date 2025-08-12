#![allow(dead_code)]

use semantic_query::{core::{LowLevelClient, QueryResolver, RetryConfig}, clients::flexible::FlexibleClient};
use serde::{Deserialize};
use schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema)]
struct SimpleResponse {
    message: String,
    success: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    println!("🧪 FlexibleClient Demo");
    println!("=================");
    
    // Create different client types easily
    println!("\n1. Creating different client types:");
    let (mock_client, _) = FlexibleClient::mock();
    
    println!("   ✅ Mock client: {:?}", mock_client);
    println!("   📝 Claude/DeepSeek clients can be created with FlexibleClient::claude() / ::deepseek()");
    
    // Clone clients easily
    println!("\n2. Cloning clients:");
    let cloned_mock = mock_client.clone();
    println!("   ✅ Cloned mock client: {:?}", cloned_mock);
    
    // Extract boxed clients for use with other systems
    println!("\n3. Extract boxed clients:");
    let _boxed_client = cloned_mock.clone_box();
    println!("   ✅ Extracted boxed client from FlexibleClient");
    
    // Use with QueryResolver
    println!("\n4. Using with QueryResolver:");
    let resolver = QueryResolver::new(mock_client, RetryConfig::default());
    
    // Try a simple query (will return empty {} from mock)
    match resolver.query_with_schema::<SimpleResponse>("Hello world".to_string()).await {
        Ok(response) => println!("   ✅ Query succeeded: {:?}", response),
        Err(e) => println!("   ❌ Query failed (expected with mock): {}", e),
    }
    
    // Demonstrate factory functions
    println!("\n5. Factory functions for dynamic client creation:");
    let mock_client2 = FlexibleClient::mock();
    println!("   ✅ Created mock client: {:?}", mock_client2);
    println!("   📝 Can also create claude/deepseek clients when API keys are available");
    
    println!("\n🎉 FlexibleClient demo completed!");
    println!("    - Easy construction with FlexibleClient::mock(), ::claude(), ::deepseek()");
    println!("    - Seamless cloning with .clone()");
    println!("    - Extract boxed clients with .clone_inner() or .into_inner()");
    println!("    - Works directly with QueryResolver");
    
    Ok(())
}
