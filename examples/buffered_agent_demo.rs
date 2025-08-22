use semantic_query::clients::flexible::{FlexibleClient, ClientType};
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::streaming::StreamItem;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use schemars::JsonSchema;

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

    let client = FlexibleClient::from_type(ClientType::DeepSeek);
    let resolver = QueryResolver::new(client, RetryConfig::default());

    let task = "Gather information about the Rust tokio runtime";
    let system_instructions = r#"
You are an assistant that thinks out loud as chat lines and emits tool calls when needed.
Rules:
- Chat: Use plain text lines
- Tool calls: Emit JSON objects (not arrays) that strictly match this schema:
  { "name": string, "args": object }
"#;

    let prompt = format!("{system}\n\nTask:\n{task}", system = system_instructions);

    let evs = resolver.stream_query::<ToolCall>(prompt).await?;
    futures_util::pin_mut!(evs);

    println!("=== Buffered Agent Demo (smart JSON handling) ===");

    let mut tool_calls = 0usize;
    let mut pending_tokens = String::new();
    let mut last_was_newline = false;

    while let Some(ev) = evs.next().await {
        match ev {
            Ok(StreamItem::Token(tok)) => {
                // Buffer tokens instead of printing immediately
                pending_tokens.push_str(&tok);
                
                // Flush tokens on sentence boundaries or when buffer gets large
                let should_flush = tok.contains('.') || 
                                  tok.contains('?') || 
                                  tok.contains('!') ||
                                  tok.contains('\n') ||
                                  pending_tokens.len() > 100;
                
                if should_flush && !pending_tokens.trim().is_empty() {
                    // Check if this looks like the start of JSON
                    let trimmed = pending_tokens.trim();
                    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                        // Not JSON, safe to print
                        print!("{}", pending_tokens);
                        std::io::Write::flush(&mut std::io::stdout()).unwrap();
                        last_was_newline = pending_tokens.ends_with('\n');
                    }
                    pending_tokens.clear();
                }
            }
            Ok(StreamItem::Text(text)) => {
                // Flush any pending tokens first
                if !pending_tokens.trim().is_empty() {
                    let trimmed = pending_tokens.trim();
                    if !trimmed.starts_with('{') && !trimmed.starts_with('[') {
                        print!("{}", pending_tokens);
                        std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    }
                    pending_tokens.clear();
                }

                let clean = text.text.trim();
                if !clean.is_empty() {
                    if !last_was_newline { println!(); }
                    println!("[agent] {}", clean);
                    last_was_newline = false;
                }
            }
            Ok(StreamItem::Data(tc)) => {
                // Clear any pending JSON tokens (don't print them)
                pending_tokens.clear();

                // Print formatted tool call
                tool_calls += 1;
                if !last_was_newline { println!(); }
                println!("{}[toolcall {}] name={}{}", COLOR_TOOL, tool_calls, tc.name, COLOR_RESET);
                println!("{}{}{}", COLOR_TOOL, pretty_json(&tc.args), COLOR_RESET);
                last_was_newline = false;
            }
            Err(e) => {
                eprintln!("\nStream error: {}", e);
                break;
            }
        }
    }

    // Flush any remaining tokens
    if !pending_tokens.trim().is_empty() {
        print!("{}", pending_tokens);
    }

    if tool_calls == 0 {
        eprintln!("⚠️  No tool calls detected.");
    } else {
        println!("\n✅ Detected {} tool call(s).", tool_calls);
    }

    Ok(())
}

fn pretty_json(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

const COLOR_TOOL: &str = "\x1b[38;5;213m";
const COLOR_RESET: &str = "\x1b[0m";