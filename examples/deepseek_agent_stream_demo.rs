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
use semantic_query::json_utils;
use tokio::io::{AsyncBufReadExt, BufReader};

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

    // Stream DeepSeek SSE and aggregate human-readable output while still printing tokens live.
    let reader = resolver.client().stream_raw_reader(prompt);
    let mut br = BufReader::new(reader).lines();

    println!("=== DeepSeek Agent Demo (IRC-style, streaming) ===");
    let mut tool_calls = 0usize;
    let mut sse_event = String::new();
    let mut text_buf = String::new();

    while let Ok(Some(line)) = br.next_line().await {
        if line.is_empty() {
            // End of one SSE event; process it
            if let Some(payload) = sse_event.strip_prefix("data: ") {
                if payload.trim() == "[DONE]" { break; }
                // Try to parse as an SSE delta frame and extract token content
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                    // choices[0].delta.content
                    if let Some(token) = v.get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c0| c0.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        // Live token output (no JSON spam)
                        print!("{}", token);
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        text_buf.push_str(token);

                        // Detect completed JSON objects inside the accumulating text
                        let coords = json_utils::find_json_structures(&text_buf);
                        // Try to parse any complete object as a ToolCall
                        // If found, flush preceding text as a chunk, then emit the tool call
                        let mut consumed_up_to = 0usize;
                        for node in coords {
                            let end = node.end.saturating_add(1); // end is inclusive in coords
                            let slice = &text_buf[node.start..end];
                            if let Ok(tc) = serde_json::from_str::<ToolCall>(slice) {
                                // Flush any preceding text chunk
                                if node.start > 0 {
                                    let chunk = text_buf[consumed_up_to..node.start].trim();
                                    if !chunk.is_empty() { println!("\n[agent] {}", chunk); }
                                }
                                tool_calls += 1;
                                println!("\n[toolcall {}] name={} args=\n{}", tool_calls, tc.name, pretty_json(&tc.args));
                                consumed_up_to = node.end; // mark JSON as consumed
                            }
                        }
                        if consumed_up_to > 0 {
                            // Remove consumed JSON and preceding text up to end of last object
                            text_buf.drain(..consumed_up_to);
                        }

                        // Paragraph flush: on double newline, emit a cleaned chunk
                        if text_buf.contains("\n\n") {
                            let parts: Vec<&str> = text_buf.splitn(2, "\n\n").collect();
                            let chunk = parts[0].trim();
                            if !chunk.is_empty() {
                                println!("\n[agent] {}", chunk);
                            }
                            text_buf = parts[1].to_string();
                        }

                        // Finish reason flush
                        if v.get("choices").and_then(|c| c.get(0)).and_then(|c0| c0.get("finish_reason")).and_then(|fr| fr.as_str()).is_some() {
                            let chunk = text_buf.trim();
                            if !chunk.is_empty() {
                                println!("\n[agent] {}", chunk);
                            }
                            text_buf.clear();
                        }
                    }
                }
            }
            sse_event.clear();
        } else {
            // Accumulate SSE event lines (some providers send multi-line JSON per event)
            if !sse_event.is_empty() { sse_event.push('\n'); }
            sse_event.push_str(&line);
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
