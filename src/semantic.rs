use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::json_utils::{find_json_structures, deserialize_stream_map, ParsedOrUnknown};
use tracing::{debug, instrument};
use tokio::io::{AsyncRead, AsyncReadExt};
use async_stream::stream;
use futures_core::stream::Stream;
use futures_util::StreamExt;
use bytes::Bytes;
use std::pin::Pin;

/// Represents a piece of unstructured text content returned by the model.
///
/// Usage:
/// - `SemanticItem::Text(TextContent { text })` preserves non-JSON content in the
///   order it appears, so you never lose commentary or context.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TextContent {
    /// Plain text content. Downstream systems can render or log this.
    pub text: String,
}

/// A semantic item in the model's response stream: token, text, or typed data `T`.
///
/// Usage:
/// - One-shot: `Vec<SemanticItem<T>>` via `QueryResolver::query_semantic`.
/// - Streaming: `Stream<Item=SemanticItem<T>>` via `stream_semantic_from_async_read`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "content")]
pub enum SemanticItem<T>
where
    T: JsonSchema,
{
    /// Individual token for real-time display
    #[serde(skip)]
    Token(String),
    /// Free-form text emitted by the model.
    Text(TextContent),
    /// Structured data conforming to the user-provided schema.
    Data(T),
}

/// Convenience alias describing the full response as an ordered stream.
pub type SemanticStream<T> = Vec<SemanticItem<T>>;

/// Construct a semantic stream from a raw model response using the streaming
/// structure parser for segmentation. Any JSON structure that deserializes to `T`
/// becomes `SemanticItem::Data(T)`. Non-matching JSON and all non-JSON text are
/// preserved as `SemanticItem::Text` in order.
/// Build a semantic stream (ordered list of Text/Data(T)) from raw text.
#[instrument(target = "semantic_query::json_stream", skip(raw))]
pub fn build_semantic_stream<T>(raw: &str) -> SemanticStream<T>
where
    T: DeserializeOwned + JsonSchema,
{
    let mut items: SemanticStream<T> = Vec::new();
    let roots = find_json_structures(raw);
    let mut cursor = 0usize;

    for node in roots {
        // Emit text before this node
        if node.start > cursor {
            let text_slice = &raw[cursor..node.start];
            let trimmed = text_slice.trim();
            if !trimmed.is_empty() {
                items.push(SemanticItem::Text(TextContent { text: text_slice.to_string() }));
            }
        }

        // Try to parse this node or any of its children that match T.
        let end = node.end + 1; // inclusive -> make end exclusive
        let json_slice = &raw[node.start..end];
        let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
        if mapped.is_empty() {
            // No structures detected inside (unlikely), preserve as text
            items.push(SemanticItem::Text(TextContent { text: json_slice.to_string() }));
        } else {
            let mut any_parsed = false;
            for item in mapped {
                match item {
                    ParsedOrUnknown::Parsed(v) => {
                        any_parsed = true;
                        items.push(SemanticItem::Data(v));
                    }
                    ParsedOrUnknown::Unknown(u) => {
                        // Preserve unknown JSON chunks as text to keep fidelity
                        let u_end = u.end + 1;
                        if u_end <= json_slice.len() && u.start < u_end {
                            let sub = &json_slice[u.start..u_end];
                            items.push(SemanticItem::Text(TextContent { text: sub.to_string() }));
                        } else {
                            debug!(target = "semantic_query::json_stream", "Skipping invalid unknown coordinates");
                        }
                    }
                }
            }
            if !any_parsed {
                // Fallback: include full slice to avoid losing info
                items.push(SemanticItem::Text(TextContent { text: json_slice.to_string() }));
            }
        }

        cursor = end;
    }

    // Emit trailing text
    if cursor < raw.len() {
        let text_slice = &raw[cursor..];
        let trimmed = text_slice.trim();
        if !trimmed.is_empty() {
            items.push(SemanticItem::Text(TextContent { text: text_slice.to_string() }));
        }
    }

    items
}

/// Stream `SemanticItem<T>` from an `AsyncRead` by incrementally parsing JSON
/// structures and interleaving free-form text between them.
///
/// Use this for realtime toolcalls or progressive UIs.
pub fn stream_semantic_from_async_read<R, T>(mut reader: R, buf_size: usize) -> impl Stream<Item = SemanticItem<T>>
where
    R: AsyncRead + Unpin + Send + 'static,
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    stream! {
        let mut parser = crate::json_utils::JsonStreamParser::new();
        let mut accum = String::new();
        let mut last_offset: usize = 0;
        let mut buf = vec![0u8; buf_size.max(1024)];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        let old_len = accum.len();
                        accum.push_str(s);
                        for node in parser.feed(s) {
                            // Emit text before node
                            if node.start > last_offset && node.start <= accum.len() {
                                let text_slice = &accum[last_offset..node.start];
                                if !text_slice.trim().is_empty() {
                                    yield SemanticItem::Text(TextContent { text: text_slice.to_string() });
                                }
                            }

                            // Process node slice
                            let end = node.end + 1;
                            if end <= accum.len() {
                                let json_slice = &accum[node.start..end];
                                let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
                                if mapped.is_empty() {
                                    yield SemanticItem::Text(TextContent { text: json_slice.to_string() });
                                } else {
                                    let mut any = false;
                                    for item in mapped {
                                        match item {
                                            ParsedOrUnknown::Parsed(v) => { any = true; yield SemanticItem::Data(v); }
                                            ParsedOrUnknown::Unknown(u) => {
                                                let u_end = u.end + 1;
                                                if u_end <= json_slice.len() && u.start < u_end {
                                                    let sub = &json_slice[u.start..u_end];
                                                    yield SemanticItem::Text(TextContent { text: sub.to_string() });
                                                }
                                            }
                                        }
                                    }
                                    if !any { yield SemanticItem::Text(TextContent { text: json_slice.to_string() }); }
                                }
                                last_offset = end;
                            }
                        }
                        let _ = old_len;
                    }
                }
                Err(_) => break,
            }
        }
        // Emit trailing text
        if last_offset < accum.len() {
            let text_slice = &accum[last_offset..];
            if !text_slice.trim().is_empty() {
                yield SemanticItem::Text(TextContent { text: text_slice.to_string() });
            }
        }
    }
}

/// Stream `SemanticItem<T>` from a bytes stream (such as from an HTTP response).
///
/// This is the high-level streaming adapter that converts raw bytes into semantic items
/// with proper error handling. It automatically handles UTF-8 conversion and incremental
/// JSON parsing without exposing low-level buffer management.
pub fn stream_semantic_from_bytes_stream<T>(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, crate::error::AIError>> + Send>>
) -> impl Stream<Item = Result<SemanticItem<T>, crate::error::QueryResolverError>>
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    stream! {
        let mut parser = crate::json_utils::JsonStreamParser::new();
        let mut accum = String::new();
        let mut last_offset: usize = 0;
        
        let mut byte_stream = byte_stream;
        while let Some(chunk_result) = byte_stream.next().await {
            match chunk_result {
                Ok(bytes) => {
                    // Convert bytes to string
                    match std::str::from_utf8(&bytes) {
                        Ok(s) => {
                            accum.push_str(s);
                            
                            // Process any complete JSON structures
                            for node in parser.feed(s) {
                                // Emit text before node
                                if node.start > last_offset && node.start <= accum.len() {
                                    let text_slice = &accum[last_offset..node.start];
                                    if !text_slice.trim().is_empty() {
                                        yield Ok(SemanticItem::Text(TextContent { text: text_slice.to_string() }));
                                    }
                                }

                                // Process node slice
                                let end = node.end + 1;
                                if end <= accum.len() {
                                    let json_slice = &accum[node.start..end];
                                    let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
                                    if mapped.is_empty() {
                                        yield Ok(SemanticItem::Text(TextContent { text: json_slice.to_string() }));
                                    } else {
                                        let mut any_parsed = false;
                                        for item in mapped {
                                            match item {
                                                ParsedOrUnknown::Parsed(v) => { 
                                                    any_parsed = true; 
                                                    yield Ok(SemanticItem::Data(v)); 
                                                }
                                                ParsedOrUnknown::Unknown(u) => {
                                                    let u_end = u.end + 1;
                                                    if u_end <= json_slice.len() && u.start < u_end {
                                                        let sub = &json_slice[u.start..u_end];
                                                        yield Ok(SemanticItem::Text(TextContent { text: sub.to_string() }));
                                                    }
                                                }
                                            }
                                        }
                                        if !any_parsed { 
                                            yield Ok(SemanticItem::Text(TextContent { text: json_slice.to_string() })); 
                                        }
                                    }
                                    last_offset = end;
                                }
                            }
                        }
                        Err(utf8_err) => {
                            yield Err(crate::error::QueryResolverError::Ai(
                                crate::error::AIError::Mock(format!("UTF-8 decode error: {}", utf8_err))
                            ));
                            break;
                        }
                    }
                }
                Err(ai_error) => {
                    yield Err(crate::error::QueryResolverError::Ai(ai_error));
                    break;
                }
            }
        }
        
        // Emit any remaining text
        if last_offset < accum.len() {
            let text_slice = &accum[last_offset..];
            if !text_slice.trim().is_empty() {
                yield Ok(SemanticItem::Text(TextContent { text: text_slice.to_string() }));
            }
        }
    }
}

/// Stream `SemanticItem<T>` from an SSE bytes stream with proper token aggregation.
///
/// This processes Server-Sent Events format and aggregates tokens from the content field
/// into semantic items. It handles the complexity of SSE parsing and JSON extraction
/// so users get clean Text/Data events.
pub fn stream_semantic_from_sse_bytes<T>(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, crate::error::AIError>> + Send>>
) -> impl Stream<Item = Result<SemanticItem<T>, crate::error::QueryResolverError>>
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    stream! {
        use crate::streaming::{stream_sse_aggregated, AggregatedEvent};
        use tokio_util::io::StreamReader;
        
        // Convert bytes stream to AsyncRead
        let io_stream = byte_stream.map(|res| match res {
            Ok(bytes) => Ok::<Bytes, std::io::Error>(bytes),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        });
        let reader = StreamReader::new(io_stream);
        
        // Use the existing SSE aggregation logic
        let aggregated_stream = stream_sse_aggregated::<_, T>(reader, 8192);
        futures_util::pin_mut!(aggregated_stream);
        
        while let Some(event) = aggregated_stream.next().await {
            match event {
                AggregatedEvent::Token(token) => {
                    // Emit individual tokens for real-time display
                    yield Ok(SemanticItem::Token(token));
                }
                AggregatedEvent::TextChunk(text) => {
                    let clean_text = text.trim();
                    if !clean_text.is_empty() {
                        yield Ok(SemanticItem::Text(TextContent { text: clean_text.to_string() }));
                    }
                }
                AggregatedEvent::Data(parsed_item) => {
                    yield Ok(SemanticItem::Data(parsed_item));
                }
            }
        }
    }
}
