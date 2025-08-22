use serde::Deserialize;
use serde_json::json;
use semantic_query::json_utils;

#[derive(Deserialize, Debug, PartialEq)]
struct ToolCall { name: String, args: serde_json::Value }

#[derive(Debug, PartialEq)]
enum Event {
    Text(String),
    Call(ToolCall),
}

fn sse_payload(token: &str) -> String {
    let payload = json!({
        "choices": [ { "delta": { "content": token } } ]
    });
    format!("data: {}", payload)
}

fn run_aggregator(lines: Vec<String>) -> Vec<Event> {
    let mut sse_event = String::new();
    let mut text_buf = String::new();
    let mut events: Vec<Event> = Vec::new();

    for line in lines {
        if line.is_empty() {
            if let Some(payload) = sse_event.strip_prefix("data: ") {
                if payload.trim() == "[DONE]" { break; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                    if let Some(token) = v.get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c0| c0.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        // Accumulate tokens
                        text_buf.push_str(token);

                        // Detect completed JSON ToolCall objects inside accumulator
                        let coords = json_utils::find_json_structures(&text_buf);
                        let mut consumed_up_to = 0usize;
                        for node in coords {
                            let end = node.end + 1; // inclusive end -> exclusive
                            let slice = &text_buf[node.start..end];
                            if let Ok(tc) = serde_json::from_str::<ToolCall>(slice) {
                                // Flush preceding text chunk
                                if node.start > 0 {
                                    let chunk = text_buf[..node.start].trim();
                                    if !chunk.is_empty() {
                                        events.push(Event::Text(chunk.to_string()));
                                    }
                                }
                                events.push(Event::Call(tc));
                                consumed_up_to = consumed_up_to.max(end);
                            }
                        }
                        if consumed_up_to > 0 {
                            text_buf.drain(..consumed_up_to);
                        }

                        // Paragraph flush (double newline)
                        if let Some(idx) = text_buf.find("\n\n") {
                            let (chunk, rest) = text_buf.split_at(idx);
                            let chunk = chunk.trim();
                            if !chunk.is_empty() {
                                events.push(Event::Text(chunk.to_string()));
                            }
                            text_buf = rest[2..].to_string();
                        }
                    }
                }
            }
            sse_event.clear();
        } else {
            if !sse_event.is_empty() { sse_event.push('\n'); }
            sse_event.push_str(&line);
        }
    }

    // Flush any trailing text
    let tail = text_buf.trim();
    if !tail.is_empty() { events.push(Event::Text(tail.to_string())); }
    events
}

#[test]
fn aggregator_detects_multiple_calls_and_text() {
    let mut lines: Vec<String> = Vec::new();
    // Intro text tokens and paragraph break
    for t in ["Intro paragraph about tokio.", "\n\n"].iter() {
        lines.push(sse_payload(t)); lines.push(String::new());
    }
    // Tool call 1 across many tokens
    for t in ["{", "\"name\":\"fetch_docs\"", ",", "\"args\":{\"q\":\"tokio runtime\"}", "}"].iter() {
        lines.push(sse_payload(t)); lines.push(String::new());
    }
    // Middle text
    for t in [" Checking crates.io stats.", "\n\n"].iter() {
        lines.push(sse_payload(t)); lines.push(String::new());
    }
    // Tool call 2 with nested args and array
    for t in [
        "{",
        "\"name\":\"fetch_repo\"",
        ",",
        "\"args\":{\"owner\":\"tokio-rs\",\"repo\":\"tokio\",\"filters\":[\"open_issues\",\"stars\"]}",
        "}"
    ].iter() {
        lines.push(sse_payload(t)); lines.push(String::new());
    }
    // Text with braces inside strings (ensure we don't mis-detect)
    for t in [" Note: \"text with { braces } inside\" should be fine.", "\n\n"].iter() {
        lines.push(sse_payload(t)); lines.push(String::new());
    }
    // DONE
    lines.push("data: [DONE]".to_string()); lines.push(String::new());

    let events = run_aggregator(lines);

    // Expect: Text("Intro paragraph about tokio."), Call(fetch_docs), Text("Checking crates.io stats."), Call(fetch_repo), Text("Note: \"text with { braces } inside\" should be fine.")
    assert!(matches!(events.first(), Some(Event::Text(s)) if s == "Intro paragraph about tokio."));
    assert!(matches!(events.get(1), Some(Event::Call(tc)) if tc.name == "fetch_docs" && tc.args["q"] == "tokio runtime"));
    assert!(matches!(events.get(2), Some(Event::Text(s)) if s == "Checking crates.io stats."));
    assert!(matches!(events.get(3), Some(Event::Call(tc)) if tc.name == "fetch_repo" && tc.args["owner"] == "tokio-rs" && tc.args["repo"] == "tokio" && tc.args["filters"][0] == "open_issues"));
    assert!(matches!(events.get(4), Some(Event::Text(s)) if s == "Note: \"text with { braces } inside\" should be fine."));
}

