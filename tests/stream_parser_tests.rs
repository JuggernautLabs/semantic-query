use semantic_query::json_utils::{find_json_structures, deserialize_stream_map, ParsedOrUnknown, JsonStreamParser};
use serde::Deserialize;

#[test]
fn find_json_structures_simple() {
    let text = "x {\"a\":1} y";
    let coords = find_json_structures(text);
    assert_eq!(coords.len(), 1);
    let obj = &coords[0];
    // Verify the slice matches the object
    let end_excl = obj.end + 1;
    assert_eq!(&text[obj.start..end_excl], "{\"a\":1}");
}

#[derive(Deserialize, Debug, PartialEq)]
struct A { a: i32 }

#[test]
fn deserialize_stream_map_parsed_and_unknown() {
    let text = "noise {\"a\":1} and {\"b\":2}";
    let items: Vec<ParsedOrUnknown<A>> = deserialize_stream_map(text);
    // We expect one Parsed(A) and one Unknown
    let parsed = items.iter().filter_map(|it| match it { ParsedOrUnknown::Parsed(a) => Some(a), _ => None }).count();
    let unknown = items.iter().filter(|it| matches!(it, ParsedOrUnknown::Unknown(_))).count();
    assert_eq!(parsed, 1);
    assert!(unknown >= 1); // at least the {"b":2} node
}

#[test]
fn json_stream_parser_across_chunks() {
    let mut p = JsonStreamParser::new();
    let mut roots = Vec::new();
    roots.extend(p.feed("prefix {\"a\""));
    assert!(roots.is_empty());
    roots.extend(p.feed(":1}"));
    assert_eq!(roots.len(), 1);
    let node = &roots[0];
    // Absolute positions should cover the full object
    // Find the reconstructed text to verify
    let full = "prefix {\"a\":1}";
    let end_excl = node.end + 1;
    assert_eq!(&full[node.start..end_excl], "{\"a\":1}");
}

// Simulate minimal SSE aggregation: accumulate token content into text, detect a ToolCall JSON
#[derive(Deserialize, Debug)]
struct ToolCall { name: String, args: serde_json::Value }

#[test]
fn sse_aggregator_detects_toolcall() {
    // Simulated SSE lines ending with blank lines between events
    let lines = vec![
        // {"name":"web_search","args":{"q":"tokio"}}
        "data: {\"choices\":[{\"delta\":{\"content\":\"{\"}}]}".to_string(),
        "".to_string(),
        "data: {\"choices\":[{\"delta\":{\"content\":\"\\\"name\\\":\\\"web_search\\\",\\\"args\\\":{\\\"q\\\":\\\"tokio\\\"}\"}}]}".to_string(),
        "".to_string(),
        "data: {\"choices\":[{\"delta\":{\"content\":\"}\"}}]}".to_string(),
        "".to_string(),
        "data: [DONE]".to_string(),
        "".to_string(),
    ];

    let mut sse_event = String::new();
    let mut text_buf = String::new();
    let mut found = false;

    for line in lines {
        if line.is_empty() {
            if let Some(payload) = sse_event.strip_prefix("data: ") {
                if payload.trim() == "[DONE]" { break; }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                    if let Some(token) = v.get("choices").and_then(|c| c.get(0)).and_then(|c0| c0.get("delta")).and_then(|d| d.get("content")).and_then(|c| c.as_str()) {
                        text_buf.push_str(token);
                        let coords = semantic_query::json_utils::find_json_structures(&text_buf);
                        for node in coords {
                            let end = node.end + 1;
                            if let Ok(tc) = serde_json::from_str::<ToolCall>(&text_buf[node.start..end]) {
                                assert_eq!(tc.name, "web_search");
                                assert_eq!(tc.args["q"], "tokio");
                                found = true;
                            }
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

    assert!(found, "expected to detect a ToolCall in SSE token stream");
}
