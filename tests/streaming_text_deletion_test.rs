use semantic_query::streaming::{StreamItem, stream_from_async_read};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
struct ToolCall {
    name: String,
    args: serde_json::Value,
}

/// Simulate the streaming behavior we see from DeepSeek
#[tokio::test]
async fn test_streaming_text_before_json() {
    let (mut tx, rx) = tokio::io::duplex(1024);
    
    // Simulate tokens coming in that form text, then JSON, then more text
    tokio::spawn(async move {
        // Text before JSON
        let _ = tx.write_all(b"I need to search for information. ").await;
        // JSON tool call
        let _ = tx.write_all(br#"{"name": "search", "args": {"query": "tokio"}}"#).await;
        // Text after JSON
        let _ = tx.write_all(b" Now let me analyze the results.").await;
    });
    
    let stream = stream_from_async_read::<_, ToolCall>(rx, 256);
    futures_util::pin_mut!(stream);
    
    let mut items = vec![];
    while let Some(item) = stream.next().await {
        items.push(item);
    }
    
    // Verify we get the expected sequence
    assert_eq!(items.len(), 3);
    
    // First should be the text before JSON
    match &items[0] {
        StreamItem::Text(t) => assert_eq!(t.text.trim(), "I need to search for information."),
        _ => panic!("Expected Text, got {:?}", items[0]),
    }
    
    // Second should be the parsed tool call
    match &items[1] {
        StreamItem::Data(tc) => {
            assert_eq!(tc.name, "search");
            assert_eq!(tc.args["query"], "tokio");
        },
        _ => panic!("Expected Data, got {:?}", items[1]),
    }
    
    // Third should be the text after JSON
    match &items[2] {
        StreamItem::Text(t) => assert_eq!(t.text.trim(), "Now let me analyze the results."),
        _ => panic!("Expected Text, got {:?}", items[2]),
    }
}

/// Test that partial text gets cut off properly when JSON is mid-sentence
#[tokio::test]
async fn test_text_cutoff_at_json_boundary() {
    let (mut tx, rx) = tokio::io::duplex(1024);
    
    tokio::spawn(async move {
        // Simulate text that gets cut off mid-word when JSON starts
        let _ = tx.write_all(b"Let me search for informati").await;
        let _ = tx.write_all(br#"{"name": "search", "args": {"query": "rust"}}"#).await;
    });
    
    let stream = stream_from_async_read::<_, ToolCall>(rx, 256);
    futures_util::pin_mut!(stream);
    
    let mut items = vec![];
    while let Some(item) = stream.next().await {
        items.push(item);
    }
    
    // We should get text "Let me search for informati" and then the tool call
    assert_eq!(items.len(), 2);
    
    match &items[0] {
        StreamItem::Text(t) => {
            // The incomplete word "informati" should still be included
            assert_eq!(t.text.trim(), "Let me search for informati");
        },
        _ => panic!("Expected Text, got {:?}", items[0]),
    }
}

/// Test multiple JSON objects in sequence
#[tokio::test] 
async fn test_multiple_json_objects() {
    let (mut tx, rx) = tokio::io::duplex(1024);
    
    tokio::spawn(async move {
        let _ = tx.write_all(b"First search: ").await;
        let _ = tx.write_all(br#"{"name": "search", "args": {"query": "tokio"}}"#).await;
        let _ = tx.write_all(b" Second search: ").await;
        let _ = tx.write_all(br#"{"name": "search", "args": {"query": "async"}}"#).await;
        let _ = tx.write_all(b" Done.").await;
    });
    
    let stream = stream_from_async_read::<_, ToolCall>(rx, 256);
    futures_util::pin_mut!(stream);
    
    let mut items = vec![];
    while let Some(item) = stream.next().await {
        items.push(item);
    }
    
    assert_eq!(items.len(), 5);
    
    // Verify the sequence
    assert!(matches!(&items[0], StreamItem::Text(t) if t.text.trim() == "First search:"));
    assert!(matches!(&items[1], StreamItem::Data(tc) if tc.name == "search" && tc.args["query"] == "tokio"));
    assert!(matches!(&items[2], StreamItem::Text(t) if t.text.trim() == "Second search:"));
    assert!(matches!(&items[3], StreamItem::Data(tc) if tc.name == "search" && tc.args["query"] == "async"));
    assert!(matches!(&items[4], StreamItem::Text(t) if t.text.trim() == "Done."));
}

/// Test SSE format with tokens coming one at a time
#[tokio::test]
async fn test_sse_token_aggregation() {
    use semantic_query::streaming::stream_from_sse_bytes;
    use bytes::Bytes;
    use futures_util::stream;
    
    // Create a stream of SSE events
    let events = vec![
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"Let \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"me \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"search \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"for \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"{\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"\\\"name\\\"\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\": \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"\\\"search\\\"\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\", \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"\\\"args\\\"\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\": \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"{\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"\\\"query\\\"\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\": \"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"\\\"test\\\"\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"}\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\"}\"}}]}\n\n")),
        Ok(Bytes::from("data: {\"choices\":[{\"delta\":{\"content\":\" done.\"}}]}\n\n")),
        Ok(Bytes::from("data: [DONE]\n\n")),
    ];
    
    let byte_stream = Box::pin(stream::iter(events));
    let stream = stream_from_sse_bytes::<ToolCall>(byte_stream);
    futures_util::pin_mut!(stream);
    
    let mut items = vec![];
    let mut tokens = vec![];
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(StreamItem::Token(t)) => tokens.push(t),
            Ok(item) => items.push(item),
            Err(e) => panic!("Stream error: {}", e),
        }
    }
    
    // Should have received all individual tokens
    assert!(!tokens.is_empty());
    let full_text = tokens.join("");
    assert!(full_text.contains("Let me search for"));
    assert!(full_text.contains(r#"{"name": "search", "args": {"query": "test"}}"#));
    assert!(full_text.contains(" done."));
    
    // Should have parsed the structure
    let has_search_call = items.iter().any(|item| {
        matches!(item, StreamItem::Data(tc) if tc.name == "search" && tc.args["query"] == "test")
    });
    assert!(has_search_call, "Should have parsed the tool call");
}