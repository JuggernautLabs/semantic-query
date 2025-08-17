//! Simple agent demo using the new high-level streaming API.
//!
//! This example demonstrates how much simpler streaming becomes with the new
//! `stream_semantic()` method. Compare this to `deepseek_agent_stream_demo.rs`
//! to see the difference in complexity.
//!
//! Configure your API key in `.env` as `DEEPSEEK_API_KEY=...`.
//! 
//! Run:
//!   cargo run --example simple_agent_stream_demo

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use semantic_query::clients::flexible::{FlexibleClient, ClientType};
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::semantic::SemanticItem;
use futures_util::StreamExt;

/// A minimal, flexible tool call representation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCall {
    /// The tool name, e.g., "web.search", "files.read", "code.run"
    pub name: String,
    /// Arbitrary JSON payload of arguments
    pub args: Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env and initialize tracing if configured
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // Create a DeepSeek client and wrap in QueryResolver
    let client = FlexibleClient::from_type(ClientType::DeepSeek);
    let resolver = QueryResolver::new(client, RetryConfig::default());

    // Simple prompt for the agent
    let prompt = r#"
You are an assistant that thinks out loud and uses tools.

Task: Gather facts about the Rust "tokio" runtime.

Rules:
- Think step-by-step with plain text
- When you need to use a tool, emit a JSON object on its own line: {"name": "tool_name", "args": {...}}
- Provide at least 2 tool calls that make sense together
- Don't wrap JSON in code blocks, just emit raw JSON objects

Example:
I need to search for information about tokio.
{"name": "web.search", "args": {"query": "tokio rust runtime"}}
Now I'll look for the official documentation.
{"name": "web.search", "args": {"query": "tokio documentation"}}

Go ahead and start thinking through this task.
"#.to_string();

    println!("🤖 Simple Agent Demo (using new streaming API)");
    println!("=================================================");

    // This is the new simple API - no pin_mut! needed!
    let mut stream = resolver.stream_semantic::<ToolCall>(prompt).await?;

    let mut tool_count = 0;
    let mut in_token_stream = false;
    
    while let Some(item_result) = stream.next().await {
        match item_result {
            Ok(SemanticItem::Token(token)) => {
                // Print tokens in real-time for live streaming effect
                print!("{}", token);
                std::io::Write::flush(&mut std::io::stdout()).ok();
                in_token_stream = true;
            }
            Ok(SemanticItem::Text(text)) => {
                // Text chunks are aggregated content, print on new line if we were streaming tokens
                if in_token_stream {
                    println!(); // End the token line
                    in_token_stream = false;
                }
                let content = text.text.trim();
                if !content.is_empty() {
                    println!("💭 {}", content);
                }
            }
            Ok(SemanticItem::Data(tool_call)) => {
                if in_token_stream {
                    println!(); // End the token line
                    in_token_stream = false;
                }
                tool_count += 1;
                println!("🔧 Tool Call #{}: {}", tool_count, tool_call.name);
                println!("   Args: {}", serde_json::to_string_pretty(&tool_call.args)?);
            }
            Err(e) => {
                eprintln!("❌ Stream error: {}", e);
                break;
            }
        }
    }
    
    // Ensure we end with a newline if still in token stream
    if in_token_stream {
        println!();
    }

    if tool_count == 0 {
        println!("\n⚠️  No tool calls detected. The model might need clearer instructions.");
    } else {
        println!("\n✅ Detected {} tool call(s) successfully!", tool_count);
    }

    Ok(())
}