use semantic_query::clients::flexible::{FlexibleClient, ClientType};
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::streaming::StreamItem;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use schemars::JsonSchema;
use std::env;

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

    println!("=== Improved Agent Demo (precise JSON deletion) ===");

    let mut tool_calls = 0usize;
    let mut current_line = String::new();
    let mut json_start_pos: Option<usize> = None;
    let mut json_depth = 0;
    let mut in_string = false;
    let mut escape = false;

    while let Some(ev) = evs.next().await {
        match ev {
            Ok(StreamItem::Token(tok)) => {
                // Track position for precise deletion
                let line_len_before = current_line.len();
                current_line.push_str(&tok);
                
                // Scan for JSON boundaries in the new token
                for (i, ch) in tok.chars().enumerate() {
                    let pos_in_line = line_len_before + i;
                    
                    if in_string {
                        if escape {
                            escape = false;
                        } else if ch == '\\' {
                            escape = true;
                        } else if ch == '"' {
                            in_string = false;
                        }
                    } else {
                        match ch {
                            '"' if json_depth > 0 => in_string = true,
                            '{' | '[' => {
                                if json_depth == 0 {
                                    // JSON starts here - mark position
                                    json_start_pos = Some(pos_in_line);
                                }
                                json_depth += 1;
                            }
                            '}' | ']' => {
                                json_depth -= 1;
                                if json_depth == 0 && json_start_pos.is_some() {
                                    // JSON complete - we'll delete it when we get the Data item
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // Print the token normally
                print!("{}", tok);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();

                // Handle newlines
                if tok.contains('\n') {
                    current_line.clear();
                    json_start_pos = None;
                }
            }
            Ok(StreamItem::Text(text)) => {
                // Only print if not already shown as tokens
                let clean = text.text.trim();
                if !clean.is_empty() {
                    println!("\n[agent] {}", clean);
                    current_line.clear();
                }
            }
            Ok(StreamItem::Data(tc)) => {
                // Precisely delete just the JSON portion
                if let Some(start_pos) = json_start_pos {
                    let json_length = current_line.len() - start_pos;
                    
                    // Move cursor back to start of JSON
                    print!("\x1b[{}D", json_length);
                    // Clear from cursor to end of line
                    print!("\x1b[K");
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    
                    // Update our line buffer
                    current_line.truncate(start_pos);
                    json_start_pos = None;
                }

                // Print formatted tool call
                tool_calls += 1;
                println!();
                println!("{}[toolcall {}] name={}{}", COLOR_TOOL, tool_calls, tc.name, COLOR_RESET);
                println!("{}{}{}", COLOR_TOOL, pretty_json(&tc.args), COLOR_RESET);
                current_line.clear();
            }
            Err(e) => {
                eprintln!("\nStream error: {}", e);
                break;
            }
        }
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