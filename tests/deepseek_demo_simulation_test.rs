/// Test that simulates the exact pattern we see in DeepSeek output
use semantic_query::streaming::{StreamItem, stream_from_sse_bytes};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use futures_util::StreamExt;
use bytes::Bytes;
use futures_util::stream;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
struct ToolCall {
    name: String,
    args: serde_json::Value,
}

fn sse_token(content: &str) -> Bytes {
    let json = serde_json::json!({
        "choices": [{
            "delta": {
                "content": content
            }
        }]
    });
    Bytes::from(format!("data: {}\n\n", json))
}

#[tokio::test]
async fn test_deepseek_cutoff_pattern() {
    // Simulate the exact pattern from the demo output
    let tokens = vec![
        // "I need to gather information about the Rust \"tokio\" runtime. Let me start by searching for recent documentation and articles about i"
        "I need to gather ",
        "information about the ",
        "Rust \"tokio\" runtime. ",
        "Let me start by ",
        "searching for recent ",
        "documentation and articles ",
        "about i", // This gets cut off!
        "\n",
        "[toolcall 1] name=",
        "web.search\n",
        "{\n",
        "  \"limit\": 5,\n", 
        "  \"query\": \"Rust ",
        "tokio runtime documentation ",
        "features\"\n",
        "}\n",
        "While that search ",
        "is running, I should ",
        "also look for the ",
        "official tokio crate ",
        "documentation to get ",
        "accurate information from ",
        "the sou", // This also gets cut off!
        "\n[toolcall 2] ",
    ];

    let events: Vec<Result<Bytes, semantic_query::error::AIError>> = tokens
        .into_iter()
        .map(|t| Ok(sse_token(t)))
        .collect();
    
    let byte_stream = Box::pin(stream::iter(events));
    let stream = stream_from_sse_bytes::<ToolCall>(byte_stream);
    futures_util::pin_mut!(stream);
    
    let mut all_tokens = String::new();
    let mut text_items = vec![];
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(t)) => {
                print!("{}", t); // Simulate live printing
                all_tokens.push_str(&t);
            },
            Ok(StreamItem::Text(text)) => {
                println!("\n[Got Text Item]: {:?}", text.text);
                text_items.push(text.text);
            },
            Ok(StreamItem::Data(tc)) => {
                println!("\n[Got Tool Call]: {}", tc.name);
            },
            Err(e) => panic!("Stream error: {}", e),
        }
    }
    
    println!("\n\nAll tokens concatenated:");
    println!("{}", all_tokens);
    
    println!("\n\nText items received:");
    for (i, text) in text_items.iter().enumerate() {
        println!("{}: {:?}", i, text);
    }
    
    // Check if we're losing the "about i" and "the sou" parts
    assert!(all_tokens.contains("about i"), "Should contain 'about i'");
    assert!(all_tokens.contains("the sou"), "Should contain 'the sou'");
}

#[tokio::test]
async fn test_json_mixed_with_text() {
    // More realistic test with actual JSON tool calls mixed in
    let content = r#"I need to search for information. {"name": "web.search", "args": {"query": "tokio runtime"}} While that's running"#;
    
    // Simulate it coming in as tokens
    let tokens: Vec<&str> = vec![
        "I need to ",
        "search for ",
        "information. ",
        "{\"name\": \"",
        "web.search\", ",
        "\"args\": {\"",
        "query\": \"tokio ",
        "runtime\"}} ",
        "While that's ",
        "running",
    ];
    
    let events: Vec<Result<Bytes, semantic_query::error::AIError>> = tokens
        .into_iter()
        .map(|t| Ok(sse_token(t)))
        .collect();
        
    let byte_stream = Box::pin(stream::iter(events));
    let stream = stream_from_sse_bytes::<ToolCall>(byte_stream);
    futures_util::pin_mut!(stream);
    
    let mut items = vec![];
    while let Some(result) = stream.next().await {
        if let Ok(item) = result {
            println!("Got item: {:?}", item);
            items.push(item);
        }
    }
    
    // We should see:
    // 1. Tokens as they arrive
    // 2. Text("I need to search for information. ")
    // 3. Data(ToolCall)
    // 4. Text(" While that's running")
    
    let text_before = items.iter().find_map(|item| {
        match item {
            StreamItem::Text(t) if t.text.contains("I need to search") => Some(&t.text),
            _ => None,
        }
    });
    
    assert!(text_before.is_some(), "Should have text before JSON");
    
    let has_tool_call = items.iter().any(|item| {
        matches!(item, StreamItem::Data(tc) if tc.name == "web.search")
    });
    assert!(has_tool_call, "Should have parsed tool call");
    
    let text_after = items.iter().find_map(|item| {
        match item {
            StreamItem::Text(t) if t.text.contains("While that's running") => Some(&t.text),
            _ => None,
        }
    });
    
    assert!(text_after.is_some(), "Should have text after JSON");
}