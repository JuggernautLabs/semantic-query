use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncBufReadExt, BufReader};
use async_stream::stream;
use futures_core::Stream;
use crate::json_utils;

#[derive(Debug, Clone, PartialEq)]
pub enum AggregatedEvent<T> {
    Token(String),
    TextChunk(String),
    Data(T),
}

/// Aggregate SSE-style streamed chat deltas into live token, text chunks, and parsed data items.
/// - Reads `AsyncRead` line-by-line as SSE (events separated by blank lines).
/// - Extracts tokens from `choices[0].delta.content` and yields `Token` events.
/// - Accumulates tokens; detects completed JSON objects and attempts to parse as `T`,
///   yielding `Data(T)` and emitting preceding text as `TextChunk`.
/// - Flushes `TextChunk` on double newline and on finish.
pub fn stream_sse_aggregated<R, T>(reader: R, _buf_size: usize) -> impl Stream<Item = AggregatedEvent<T>>
where
    R: AsyncRead + Send + Unpin + 'static,
    T: DeserializeOwned + Send + 'static,
{
    stream! {
        let mut br = BufReader::new(reader).lines();
        let mut sse_event = String::new();
        let mut text_buf = String::new();
        // Track whether we're inside a JSON structure being streamed
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escape = false;

        while let Ok(Some(line)) = br.next_line().await {
            if line.is_empty() {
                // process event
                if let Some(payload) = sse_event.strip_prefix("data: ") {
                    if payload.trim() == "[DONE]" {
                        let tail = text_buf.trim();
                        if !tail.is_empty() { yield AggregatedEvent::TextChunk(tail.to_string()); }
                        break;
                    }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                        if let Some(token) = v.get("choices").and_then(|c| c.get(0))
                            .and_then(|c0| c0.get("delta")).and_then(|d| d.get("content")).and_then(|c| c.as_str())
                        {
                            // Emit raw token for live rendering and accumulate for parsing
                            yield AggregatedEvent::Token(token.to_string());
                            text_buf.push_str(token);

                            // detect completed JSON for T
                            let coords = json_utils::find_json_structures(&text_buf);
                            let mut consumed_up_to = 0usize;
                            for node in coords {
                                let end = node.end.saturating_add(1);
                                let slice = &text_buf[node.start..end];
                                if let Ok(item) = serde_json::from_str::<T>(slice) {
                                    if node.start > 0 {
                                        let chunk = text_buf[..node.start].trim();
                                        if !chunk.is_empty() { yield AggregatedEvent::TextChunk(chunk.to_string()); }
                                    }
                                    yield AggregatedEvent::Data(item);
                                    consumed_up_to = consumed_up_to.max(end);
                                }
                            }
                            if consumed_up_to > 0 { text_buf.drain(..consumed_up_to); }

                            // Paragraph flush
                            if let Some(idx) = text_buf.find("\n\n") {
                                let (chunk, rest) = text_buf.split_at(idx);
                                let chunk = chunk.trim();
                                if !chunk.is_empty() { yield AggregatedEvent::TextChunk(chunk.to_string()); }
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
                                if !tail.is_empty() { yield AggregatedEvent::TextChunk(tail.to_string()); }
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
