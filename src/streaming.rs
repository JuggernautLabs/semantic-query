use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::json_utils::{find_json_structures, deserialize_stream_map, ParsedOrUnknown};
use tracing::{debug, instrument};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncBufReadExt, BufReader};
use async_stream::stream;
use futures_core::stream::Stream;
use futures_util::StreamExt;
use bytes::Bytes;
use std::pin::Pin;

/// Represents a piece of unstructured text content returned by the model.
///
/// Usage:
/// - `StreamItem::Text(TextContent { text })` preserves non-JSON content in the
///   order it appears, so you never lose commentary or context.
#[derive(Debug, Clone, Deserialize, JsonSchema, PartialEq)]
pub struct TextContent {
    /// Plain text content. Downstream systems can render or log this.
    pub text: String,
}

/// An item in the model's response stream: token, text, or typed data `T`.
///
/// Usage:
/// - One-shot: `Vec<StreamItem<T>>` via `QueryResolver::query_stream`.
/// - Streaming: `Stream<Item=StreamItem<T>>` via `stream_from_async_read`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "content")]
pub enum StreamItem<T>
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
pub type ParsedStream<T> = Vec<StreamItem<T>>;

/// Construct a parsed stream from a raw model response using the streaming
/// structure parser for segmentation. Any JSON structure that deserializes to `T`
/// becomes `StreamItem::Data(T)`. Non-matching JSON and all non-JSON text are
/// preserved as `StreamItem::Text` in order.
/// Build a parsed stream (ordered list of Text/Data(T)) from raw text.
#[instrument(target = "semantic_query::json_stream", skip(raw))]
pub fn build_parsed_stream<T>(raw: &str) -> ParsedStream<T>
where
    T: DeserializeOwned + JsonSchema,
{
    let mut items: ParsedStream<T> = Vec::new();
    let roots = find_json_structures(raw);
    let mut cursor = 0usize;

    for node in roots {
        // Emit text before this node
        if node.start > cursor {
            let text_slice = &raw[cursor..node.start];
            let trimmed = text_slice.trim();
            if !trimmed.is_empty() {
                items.push(StreamItem::Text(TextContent { text: text_slice.to_string() }));
            }
        }

        // Try to parse this node or any of its children that match T.
        let end = node.end + 1; // inclusive -> make end exclusive
        let json_slice = &raw[node.start..end];
        let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
        if mapped.is_empty() {
            // No structures detected inside (unlikely), preserve as text
            items.push(StreamItem::Text(TextContent { text: json_slice.to_string() }));
        } else {
            let mut any_parsed = false;
            for item in mapped {
                match item {
                    ParsedOrUnknown::Parsed(v) => {
                        any_parsed = true;
                        items.push(StreamItem::Data(v));
                    }
                    ParsedOrUnknown::Unknown(u) => {
                        // Preserve unknown JSON chunks as text to keep fidelity
                        let u_end = u.end + 1;
                        if u_end <= json_slice.len() && u.start < u_end {
                            let sub = &json_slice[u.start..u_end];
                            items.push(StreamItem::Text(TextContent { text: sub.to_string() }));
                        } else {
                            debug!(target = "semantic_query::json_stream", "Skipping invalid unknown coordinates");
                        }
                    }
                }
            }
            if !any_parsed {
                // Fallback: include full slice to avoid losing info
                items.push(StreamItem::Text(TextContent { text: json_slice.to_string() }));
            }
        }

        cursor = end;
    }

    // Emit trailing text
    if cursor < raw.len() {
        let text_slice = &raw[cursor..];
        let trimmed = text_slice.trim();
        if !trimmed.is_empty() {
            items.push(StreamItem::Text(TextContent { text: text_slice.to_string() }));
        }
    }

    items
}

/// Stream `StreamItem<T>` from an `AsyncRead` by incrementally parsing JSON
/// structures and interleaving free-form text between them.
///
/// Use this for realtime toolcalls or progressive UIs.
pub fn stream_from_async_read<R, T>(mut reader: R, buf_size: usize) -> impl Stream<Item = StreamItem<T>>
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
                                    yield StreamItem::Text(TextContent { text: text_slice.to_string() });
                                }
                            }

                            // Process node slice
                            let end = node.end + 1;
                            if end <= accum.len() {
                                let json_slice = &accum[node.start..end];
                                let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
                                if mapped.is_empty() {
                                    yield StreamItem::Text(TextContent { text: json_slice.to_string() });
                                } else {
                                    let mut any = false;
                                    for item in mapped {
                                        match item {
                                            ParsedOrUnknown::Parsed(v) => { any = true; yield StreamItem::Data(v); }
                                            ParsedOrUnknown::Unknown(u) => {
                                                let u_end = u.end + 1;
                                                if u_end <= json_slice.len() && u.start < u_end {
                                                    let sub = &json_slice[u.start..u_end];
                                                    yield StreamItem::Text(TextContent { text: sub.to_string() });
                                                }
                                            }
                                        }
                                    }
                                    if !any { yield StreamItem::Text(TextContent { text: json_slice.to_string() }); }
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
                yield StreamItem::Text(TextContent { text: text_slice.to_string() });
            }
        }
    }
}

/// Stream `StreamItem<T>` from a bytes stream (such as from an HTTP response).
///
/// This is the high-level streaming adapter that converts raw bytes into stream items
/// with proper error handling. It automatically handles UTF-8 conversion and incremental
/// JSON parsing without exposing low-level buffer management.
pub fn stream_from_bytes<T>(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, crate::error::AIError>> + Send>>
) -> impl Stream<Item = Result<StreamItem<T>, crate::error::QueryResolverError>>
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
                                        yield Ok(StreamItem::Text(TextContent { text: text_slice.to_string() }));
                                    }
                                }

                                // Process node slice
                                let end = node.end + 1;
                                if end <= accum.len() {
                                    let json_slice = &accum[node.start..end];
                                    let mapped: Vec<ParsedOrUnknown<T>> = deserialize_stream_map::<T>(json_slice);
                                    if mapped.is_empty() {
                                        yield Ok(StreamItem::Text(TextContent { text: json_slice.to_string() }));
                                    } else {
                                        let mut any_parsed = false;
                                        for item in mapped {
                                            match item {
                                                ParsedOrUnknown::Parsed(v) => { 
                                                    any_parsed = true; 
                                                    yield Ok(StreamItem::Data(v)); 
                                                }
                                                ParsedOrUnknown::Unknown(u) => {
                                                    let u_end = u.end + 1;
                                                    if u_end <= json_slice.len() && u.start < u_end {
                                                        let sub = &json_slice[u.start..u_end];
                                                        yield Ok(StreamItem::Text(TextContent { text: sub.to_string() }));
                                                    }
                                                }
                                            }
                                        }
                                        if !any_parsed { 
                                            yield Ok(StreamItem::Text(TextContent { text: json_slice.to_string() })); 
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
                yield Ok(StreamItem::Text(TextContent { text: text_slice.to_string() }));
            }
        }
    }
}

/// Stream `StreamItem<T>` from an SSE bytes stream with proper token aggregation.
///
/// This processes Server-Sent Events format and aggregates tokens from the content field
/// into stream items. It handles the complexity of SSE parsing and JSON extraction
/// so users get clean Text/Data events.
pub fn stream_from_sse_bytes<T>(
    byte_stream: Pin<Box<dyn Stream<Item = Result<Bytes, crate::error::AIError>> + Send>>
) -> impl Stream<Item = Result<StreamItem<T>, crate::error::QueryResolverError>>
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    stream! {
        use tokio_util::io::StreamReader;
        
        // Convert bytes stream to AsyncRead
        let io_stream = byte_stream.map(|res| match res {
            Ok(bytes) => Ok::<Bytes, std::io::Error>(bytes),
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())),
        });
        let reader = StreamReader::new(io_stream);
        
        // Process SSE stream
        let mut br = BufReader::new(reader).lines();
        let mut sse_event = String::new();
        let mut text_buf = String::new();
        
        while let Ok(Some(line)) = br.next_line().await {
            if line.is_empty() {
                // process event
                if let Some(payload) = sse_event.strip_prefix("data: ") {
                    if payload.trim() == "[DONE]" {
                        let tail = text_buf.trim();
                        if !tail.is_empty() { 
                            yield Ok(StreamItem::Text(TextContent { text: tail.to_string() })); 
                        }
                        break;
                    }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                        if let Some(token) = v.get("choices").and_then(|c| c.get(0))
                            .and_then(|c0| c0.get("delta")).and_then(|d| d.get("content")).and_then(|c| c.as_str())
                        {
                            // Emit raw token for live rendering and accumulate for parsing
                            yield Ok(StreamItem::Token(token.to_string()));
                            text_buf.push_str(token);

                            // detect completed JSON for T
                            let coords = find_json_structures(&text_buf);
                            let mut consumed_up_to = 0usize;
                            for node in coords {
                                let end = node.end.saturating_add(1);
                                let slice = &text_buf[node.start..end];
                                if let Ok(item) = serde_json::from_str::<T>(slice) {
                                    if node.start > 0 {
                                        let chunk = text_buf[..node.start].trim();
                                        if !chunk.is_empty() { 
                                            yield Ok(StreamItem::Text(TextContent { text: chunk.to_string() })); 
                                        }
                                    }
                                    yield Ok(StreamItem::Data(item));
                                    consumed_up_to = consumed_up_to.max(end);
                                }
                            }
                            if consumed_up_to > 0 { text_buf.drain(..consumed_up_to); }

                            // Paragraph flush
                            if let Some(idx) = text_buf.find("\n\n") {
                                let (chunk, rest) = text_buf.split_at(idx);
                                let chunk = chunk.trim();
                                if !chunk.is_empty() { 
                                    yield Ok(StreamItem::Text(TextContent { text: chunk.to_string() })); 
                                }
                                text_buf = rest[2..].to_string();
                            }

                            // Finish flush only when finish_reason is a non-null string
                            if v
                                .get("choices").and_then(|c| c.get(0))
                                .and_then(|c0| c0.get("finish_reason"))
                                .and_then(|fr| fr.as_str())
                                .is_some()
                            {
                                let tail = text_buf.trim();
                                if !tail.is_empty() { 
                                    yield Ok(StreamItem::Text(TextContent { text: tail.to_string() })); 
                                }
                                text_buf.clear();
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
    }
}
