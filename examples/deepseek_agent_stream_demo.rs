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
- Chat: Use plain text lines
- Tool calls: Emit JSON objects (not arrays) that strictly match this schema:
  { "name": string, "args": object }
- You may interleave chat and tool calls in any order.
- Do not wrap JSON in code fences.
- Provide at least two coherent tool calls in sequence that make sense together.
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
    let mut printed_live = false;
    let mut last_was_newline = false;
    // Track JSON block lines to enable erase-on-parse behavior
    let mut json_depth: i32 = 0;
    let mut json_in_string = false;
    let mut json_escape = false;
    let mut json_lines_current: usize = 0;  // lines printed in current JSON block (at least 1 if started)
    let mut json_lines_pending: usize = 0;  // lines to clear when Data(ToolCall) arrives
    while let Some(ev) = evs.next().await {
        match ev {
            AggregatedEvent::Token(tok) => {
                // Normalize whitespace in live token stream:
                // - drop pure whitespace tokens except at most a single newline
                // - collapse consecutive newlines
                let t = tok.replace("\r\n", "\n");
                if t.chars().all(|c| c.is_whitespace()) {
                    if t.contains('\n') && !last_was_newline {
                        print!("\n");
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                        last_was_newline = true;
                        printed_live = true;
                    }
                    continue;
                }
                let t2 = t.replace("\n\n", "\n");
                print!("{}", t2);
                let _ = std::io::Write::flush(&mut std::io::stdout());
                last_was_newline = t2.ends_with('\n');
                printed_live = true;

                // Update JSON scanning state based on the raw token content `tok`
                let mut depth_before = json_depth;
                for ch in tok.chars() {
                    let b = ch as u32 as u8;
                    if json_in_string {
                        if json_escape { json_escape = false; }
                        else if b == b'\\' { json_escape = true; }
                        else if b == b'"' { json_in_string = false; }
                    } else {
                        match b {
                            b'"' if json_depth > 0 => { json_in_string = true; }
                            b'{' | b'[' => { json_depth += 1; }
                            b'}' | b']' => { json_depth -= 1; }
                            _ => {}
                        }
                    }
                }
                if depth_before == 0 && json_depth > 0 && json_lines_current == 0 {
                    // JSON started; count current line as 1
                    json_lines_current = 1;
                }
                // Count additional newlines printed for this token while in a JSON block
                if depth_before > 0 || json_depth > 0 {
                    let added = t2.matches('\n').count();
                    json_lines_current = json_lines_current.saturating_add(added);
                }
                // If JSON just ended in this token, stage lines for pending clear on Data
                if depth_before > 0 && json_depth == 0 && json_lines_current > 0 {
                    json_lines_pending = json_lines_current;
                    json_lines_current = 0;
                }
            }
            AggregatedEvent::TextChunk(chunk) => {
                let clean = chunk.trim();
                if clean.is_empty() { printed_live = false; continue; }
                // If we already streamed these tokens, skip re-printing the aggregated text
                if printed_live { printed_live = false; continue; }
                // strip common chat prefixes like "< " if present
                let clean = clean.trim_start_matches("< ");
                if !last_was_newline { println!(""); }
                println!("[agent] {}", clean);
                last_was_newline = false;
            }
            AggregatedEvent::Data(tc) => {
                if printed_live { printed_live = false; }
                tool_calls += 1;
                if !last_was_newline { println!(""); }
                // Colorize tool calls for readability
                println!("{}[toolcall {}] name={}{}", COLOR_TOOL, tool_calls, tc.name, COLOR_RESET);
                println!("{}{}{}", COLOR_TOOL, pretty_json(&tc.args), COLOR_RESET);
                last_was_newline = false;
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

// Simple ANSI color helpers for nicer demo output
const COLOR_TOOL: &str = "\x1b[38;5;213m"; // pink-ish
const COLOR_RESET: &str = "\x1b[0m";
