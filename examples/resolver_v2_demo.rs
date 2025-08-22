//! Demonstration of QueryResolver V2 improvements over V1
//!
//! This example shows how V2 handles mixed content (text + structured data) 
//! more gracefully than V1, which assumes the entire response is parseable as T.

use semantic_query::clients::mock::MockClient;
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::ResponseItem;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct Analysis {
    topic: String,
    key_points: Vec<String>,
    confidence: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // Use mock client for predictable demo output
    let (client, handle) = MockClient::new();
    
    // Set up a realistic mixed-content response
    handle.add_response(semantic_query::clients::MockResponse::Success(r#"
I'll analyze the Rust async ecosystem for you. Let me break this down systematically.

The async landscape in Rust has evolved significantly. Here's my analysis:

{
  "topic": "Rust Async Ecosystem",
  "key_points": [
    "Tokio dominates the runtime space",
    "async/await syntax is stable and mature",
    "Pin and Future traits are core abstractions"
  ],
  "confidence": 0.92
}

Additionally, I should mention that there are alternative runtimes worth considering:

{
  "topic": "Alternative Runtimes",
  "key_points": [
    "async-std provides stdlib-like APIs",
    "smol is lightweight and simple",
    "Embassy targets embedded systems"
  ],
  "confidence": 0.85
}

This covers the main landscape. The ecosystem is quite mature now, with tokio being the most widely adopted solution for most use cases.
"#.trim().to_string()));

    // Add more responses for subsequent V2 calls
    handle.add_response(semantic_query::clients::MockResponse::Success(r#"
I'll analyze the Rust async ecosystem for you. Let me break this down systematically.

The async landscape in Rust has evolved significantly. Here's my analysis:

{
  "topic": "Rust Async Ecosystem",
  "key_points": [
    "Tokio dominates the runtime space",
    "async/await syntax is stable and mature", 
    "Pin and Future traits are core abstractions"
  ],
  "confidence": 0.92
}

Additionally, I should mention that there are alternative runtimes worth considering:

{
  "topic": "Alternative Runtimes",
  "key_points": [
    "async-std provides stdlib-like APIs",
    "smol is lightweight and simple",
    "Embassy targets embedded systems"
  ],
  "confidence": 0.85
}

This covers the main landscape. The ecosystem is quite mature now, with tokio being the most widely adopted solution for most use cases.
"#.trim().to_string()));

    handle.add_response(semantic_query::clients::MockResponse::Success(r#"
I'll analyze the Rust async ecosystem for you. Let me break this down systematically.

The async landscape in Rust has evolved significantly. Here's my analysis:

{
  "topic": "Rust Async Ecosystem",
  "key_points": [
    "Tokio dominates the runtime space",
    "async/await syntax is stable and mature",
    "Pin and Future traits are core abstractions"
  ],
  "confidence": 0.92
}

Additionally, I should mention that there are alternative runtimes worth considering:

{
  "topic": "Alternative Runtimes", 
  "key_points": [
    "async-std provides stdlib-like APIs",
    "smol is lightweight and simple", 
    "Embassy targets embedded systems"
  ],
  "confidence": 0.85
}

This covers the main landscape. The ecosystem is quite mature now, with tokio being the most widely adopted solution for most use cases.
"#.trim().to_string()));

    println!("=== QueryResolver Legacy vs New API Comparison ===\n");

    // Legacy methods (now deprecated stubs)
    let resolver = QueryResolver::new(client.clone(), RetryConfig::default());
    
    println!("🔍 Legacy method (query_with_schema) - now deprecated:");
    match resolver.query_with_schema::<Analysis>("Analyze the Rust async ecosystem".to_string()).await {
        Ok(analysis) => {
            println!("✅ Got analysis: {}", analysis.topic);
        }
        Err(e) => {
            println!("❌ Legacy method failed (expected - it's now a stub): {}", e);
            println!("   This demonstrates that old methods are deprecated stubs");
        }
    }

    println!("\n{}\n", "=".repeat(60));

    // New unified QueryResolver with mixed content support
    
    println!("🔍 New QueryResolver (query_mixed):");
    match resolver.query_mixed::<Analysis>("Analyze the Rust async ecosystem".to_string()).await {
        Ok(result) => {
            let data = result.data_only();
            println!("✅ Found {} analyses in mixed content:", data.len());
            
            for (i, analysis) in data.iter().enumerate() {
                println!("   {}. {}", i+1, analysis.topic);
                println!("      Points: {:?}", analysis.key_points);
                println!("      Confidence: {}", analysis.confidence);
            }
            
            println!("\n📝 Explanatory text context:");
            println!("   {}", result.text_content());
            
            println!("\n📋 Full mixed content breakdown:");
            for (i, item) in result.items.iter().enumerate() {
                match item {
                    ResponseItem::Text(t) => println!("   {}: Text({} chars)", i+1, t.text.len()),
                    ResponseItem::Data { data: d, .. } => println!("   {}: Data({})", i+1, d.topic),
                }
            }
        }
        Err(e) => println!("❌ V2 failed: {}", e),
    }

    println!("\n{}\n", "=".repeat(60));

    println!("🔍 New QueryResolver (query with schema):");
    match resolver.query::<Analysis>("Analyze the Rust async ecosystem".to_string()).await {
        Ok(result) => {
            println!("✅ Extracted {} structured analyses:", result.data_count());
            for analysis in result.data_only() {
                println!("   - {} (confidence: {})", analysis.topic, analysis.confidence);
            }
            println!("✅ Preserved context: {} chars of explanatory text", result.text_content().len());
        }
        Err(e) => println!("❌ New query failed: {}", e),
    }

    println!("\n🎯 New QueryResolver Advantages:");
    println!("   • Preserves ALL content (text + data) in order");
    println!("   • Extracts MULTIPLE structured items, not just the first");
    println!("   • Provides context for better error reporting");
    println!("   • More honest about what LLMs actually return");
    println!("   • Legacy methods are deprecated stubs - clean migration path");

    Ok(())
}