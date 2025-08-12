//! DeepSeek agent-style demo: interleaved chat + tool calls (logged, not executed).
//!
//! This example uses the DeepSeek provider via `FlexibleClient` and renders the
//! model output like an IRC chat. Plain text is shown as chat lines; any JSON
//! objects matching the `ToolCall` schema are parsed and shown as tool call logs.
//!
//! Configure your API key in `.env` as `DEEPSEEK_API_KEY=...`.
//! Optional logging via `.env`:
//!   RUST_LOG=semantic_query::json_stream=trace,semantic_query::resolver=debug
//!
//! Run:
//!   cargo run --example deepseek_agent_stream_demo

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use semantic_query::clients::flexible::{FlexibleClient, ClientType};
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::streaming::{AggregatedEvent, stream_sse_aggregated};
use futures_util::{StreamExt, pin_mut};

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

    // Prompt: ask the model to behave like an agent emitting interleaved chat
    // and JSON tool calls with a strict schema for each call
    let system_instructions = r#"
You are an assistant that thinks out loud as chat lines and emits tool calls when needed.
Rules:
- Chat: Use short plain text lines that read like IRC (no JSON).
- Tool calls: Emit JSON objects (not arrays) that strictly match this schema:
  { "name": string, "args": object }
- You may interleave chat and tool calls in any order.
- Do not wrap JSON in code fences.
- Provide at least two coherent tool calls in sequence that make sense together.
- Do not execute tools; only emit the JSON objects.
"#;

    let task = r#"
Goal: Gather facts about the Rust "tokio" runtime and summarize them.
Approach: Think step-by-step, chat your thoughts, and propose tool calls.
"#;

    let prompt = format!(
        "{system}\n\nTask:\n{task}\n\nRemember: interleave IRC-like chat lines with JSON tool calls that match the schema.",
        system = system_instructions,
        task = task
    );

    // Stream SSE via reusable aggregator
    let reader = resolver.client().stream_raw_reader(prompt);
    let evs = stream_sse_aggregated::<_, ToolCall>(reader, 8 * 1024);
    pin_mut!(evs);

    println!("=== DeepSeek Agent Demo (IRC-style, streaming) ===");
    let mut tool_calls = 0usize;
    while let Some(ev) = evs.next().await {
        match ev {
            AggregatedEvent::Token(tok) => {
                print!("{}", tok);
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            AggregatedEvent::TextChunk(chunk) => {
                if !chunk.trim().is_empty() { println!("\n[agent] {}", chunk.trim()); }
            }
            AggregatedEvent::Data(tc) => {
                tool_calls += 1;
                println!("\n[toolcall {}] name={} args=\n{}", tool_calls, tc.name, pretty_json(&tc.args));
            }
        }
    }

    if tool_calls == 0 {
        eprintln!("⚠️  No tool calls detected. Try increasing guidance or temperature settings.");
    } else {
    println!("\n✅ Detected {} tool call(s).", tool_calls);
    }

    Ok(())
}

fn pretty_json(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}
