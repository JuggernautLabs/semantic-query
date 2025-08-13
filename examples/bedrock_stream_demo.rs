//! Bedrock streaming demo for Claude via AWS SDK (feature-gated).
//! Build with: `cargo run --example bedrock_stream_demo --features aws-bedrock-sdk,bedrock,anthropic`
//! Ensure AWS credentials and a region are available (e.g., AWS_REGION),
//! or pass region in ClaudeConfig::bedrock.

use semantic_query::clients::flexible::FlexibleClient;
use semantic_query::clients::claude::{ClaudeConfig, ClaudeModel};
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::semantic::{SemanticItem, TextContent};
use futures_util::{StreamExt, pin_mut};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct ToolCall { name: String, args: serde_json::Value }

#[cfg(all(feature = "aws-bedrock-sdk", feature = "bedrock", feature = "anthropic"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // Configure Claude via Bedrock (set your desired region)
    let config = ClaudeConfig::bedrock("us-east-1".to_string(), ClaudeModel::Haiku35);
    let client = FlexibleClient::claude(config);
    let resolver = QueryResolver::new(client, RetryConfig::default());

    // Prompt that may interleave chat and JSON tool calls
    let prompt = r#"
You are an assistant that chats and emits tool calls when needed.
- Chat: plain text lines.
- Tool call: JSON object {"name": string, "args": object}.
- Interleave as needed; do not wrap JSON in code fences.
"#.to_string();

    // Stream from Bedrock over the same LowLevelClient interface
    let reader = resolver.client().stream_raw_reader(prompt);
    let stream = resolver.query_semantic_stream::<ToolCall, _>(reader, 8 * 1024);
    pin_mut!(stream);

    println!("=== Claude over Bedrock (streaming) ===");
    let mut n = 0usize;
    while let Some(item) = stream.next().await {
        match item {
            SemanticItem::Text(TextContent { text }) => {
                let s = text.trim();
                if !s.is_empty() { println!("[agent] {}", s); }
            }
            SemanticItem::Data(tc) => {
                n += 1;
                println!("[toolcall {}] {}\n{}", n, tc.name, serde_json::to_string_pretty(&tc.args).unwrap_or_default());
            }
        }
    }

    if n == 0 { eprintln!("⚠️  No toolcalls detected. Try adjusting prompt/temperature."); }
    Ok(())
}

#[cfg(not(all(feature = "aws-bedrock-sdk", feature = "bedrock", feature = "anthropic")))]
fn main() {
    eprintln!("This example requires features: aws-bedrock-sdk, bedrock, anthropic");
}
