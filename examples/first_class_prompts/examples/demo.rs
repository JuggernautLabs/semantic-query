use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use futures_util::{StreamExt, pin_mut};

use first_class_prompts::PromptSpec;
use semantic_query::clients::flexible::{FlexibleClient, ClientType};
use semantic_query::streaming::AggregatedEvent;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // Build a first-class prompt spec
    let spec = PromptSpec::<ToolCall>::semantic_interleave_v1(
        "You think aloud and emit tool calls when needed.",
        "Gather facts about the Rust 'tokio' runtime."
    );

    // Create a client (auto-select by env) and stream via the spec
    let client = FlexibleClient::from_type(ClientType::DeepSeek);
    let evs = spec.stream_events_with_client(client)?;
    pin_mut!(evs);

    println!("=== First-class Prompt Demo (tokens + parsed data) ===");
    let mut tool_count = 0usize;
    while let Some(ev) = evs.next().await {
        match ev? {
            AggregatedEvent::Token(tok) => print!("{}", tok),
            AggregatedEvent::TextChunk(chunk) => println!("\n[agent] {}", chunk.trim()),
            AggregatedEvent::Data(tc) => {
                tool_count += 1;
                println!("\n[toolcall {}] {}\n{}", tool_count, tc.name, serde_json::to_string_pretty(&tc.args)?);
            }
        }
    }

    println!("\nDetected {} tool call(s).", tool_count);
    Ok(())
}
