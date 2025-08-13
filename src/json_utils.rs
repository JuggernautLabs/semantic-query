use serde::{Serialize, Deserialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc;
use async_stream::stream;
use futures_core::stream::Stream;
use tracing::{debug, trace, instrument};

// All older sanitization/extraction helpers removed in favor of streaming parser.

// =============== Streaming JSON structure discovery ===============

/// Type of a JSON node found by the stream parser.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeType {
    Object,
    Array,
}

/// Coordinates of a JSON structure within a larger text, including nested children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjCoords {
    pub start: usize,
    pub end: usize, // inclusive index of the closing bracket/brace
    pub kind: NodeType,
    pub children: Vec<ObjCoords>,
}

impl ObjCoords {
    pub fn new(start: usize, end: usize, kind: NodeType, children: Vec<ObjCoords>) -> Self {
        Self { start, end, kind, children }
    }
}

#[derive(Debug)]
struct Frame {
    start: usize,
    kind: NodeType,
    children: Vec<ObjCoords>,
}

/// Find all JSON object/array structures in the given text. Coordinates are byte indices.
#[instrument(target = "semantic_query::json_stream", skip(text))]
pub fn find_json_structures(text: &str) -> Vec<ObjCoords> {
    let bytes = text.as_bytes();
    let mut results: Vec<ObjCoords> = Vec::new();
    let mut stack: Vec<Frame> = Vec::new();

    let mut in_string = false;
    let mut escape = false;

    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match b {
                b'\\' => escape = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match b {
            b'"' => in_string = true,
            b'{' => stack.push(Frame { start: i, kind: NodeType::Object, children: Vec::new() }),
            b'[' => stack.push(Frame { start: i, kind: NodeType::Array, children: Vec::new() }),
            b'}' => {
                if let Some(frame) = stack.pop() {
                    if frame.kind == NodeType::Object {
                        let node = ObjCoords::new(frame.start, i, NodeType::Object, frame.children);
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            results.push(node);
                        }
                    } else {
                        // Unbalanced brace
                    }
                }
            }
            b']' => {
                if let Some(frame) = stack.pop() {
                    if frame.kind == NodeType::Array {
                        let node = ObjCoords::new(frame.start, i, NodeType::Array, frame.children);
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(node);
                        } else {
                            results.push(node);
                        }
                    } else {
                        // Unbalanced bracket
                    }
                }
            }
            _ => {}
        }
    }

    debug!(target = "semantic_query::json_stream", count = results.len(), "found root structures");
    results
}

/// Stateful incremental stream parser that can be fed chunks and yields closed root nodes per feed.
#[derive(Debug, Default)]
pub struct JsonStreamParser {
    stack: Vec<Frame>,
    in_string: bool,
    escape: bool,
    /// Absolute offset (bytes) from the beginning of the full stream to the start of current chunk
    offset: usize,
}

impl JsonStreamParser {
    pub fn new() -> Self { Self::default() }

    /// Feed a new chunk. Returns any fully-closed root nodes found in this chunk.
    #[instrument(target = "semantic_query::json_stream", skip(self, chunk), fields(chunk_len = chunk.len(), offset = self.offset))]
    pub fn feed(&mut self, chunk: &str) -> Vec<ObjCoords> {
        let bytes = chunk.as_bytes();
        let mut roots: Vec<ObjCoords> = Vec::new();

        for (i, &b) in bytes.iter().enumerate() {
            let idx = self.offset + i;

            if self.in_string {
                if self.escape {
                    self.escape = false;
                    continue;
                }
                match b {
                    b'\\' => self.escape = true,
                    b'"' => self.in_string = false,
                    _ => {}
                }
                continue;
            }

            match b {
                b'"' => self.in_string = true,
                b'{' => self.stack.push(Frame { start: idx, kind: NodeType::Object, children: Vec::new() }),
                b'[' => self.stack.push(Frame { start: idx, kind: NodeType::Array, children: Vec::new() }),
                b'}' => {
                    if let Some(frame) = self.stack.pop() {
                        if frame.kind == NodeType::Object {
                            let node = ObjCoords::new(frame.start, idx, NodeType::Object, frame.children);
                            if let Some(parent) = self.stack.last_mut() {
                                parent.children.push(node);
                            } else {
                                roots.push(node);
                            }
                        }
                    }
                }
                b']' => {
                    if let Some(frame) = self.stack.pop() {
                        if frame.kind == NodeType::Array {
                            let node = ObjCoords::new(frame.start, idx, NodeType::Array, frame.children);
                            if let Some(parent) = self.stack.last_mut() {
                                parent.children.push(node);
                            } else {
                                roots.push(node);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.offset += bytes.len();
        debug!(target = "semantic_query::json_stream", roots = roots.len(), new_offset = self.offset, "feed complete");
        roots
    }
}

/// A deserialized item or an unknown structure (with coordinates) for upstream handling.
#[derive(Debug, Clone)]
pub enum ParsedOrUnknown<T> {
    Parsed(T),
    Unknown(ObjCoords),
}

/// Attempt to deserialize a node; if it fails, recursively try children.
fn descend_deserialize<T: DeserializeOwned>(text: &str, node: &ObjCoords, out: &mut Vec<ParsedOrUnknown<T>>) {
    let slice_end = node.end + 1; // end is inclusive
    let candidate = &text[node.start..slice_end];
    if let Ok(parsed) = serde_json::from_str::<T>(candidate) {
        out.push(ParsedOrUnknown::Parsed(parsed));
        return; // success: do not attempt internals
    }
    // Try children
    let before_len = out.len();
    for child in &node.children {
        descend_deserialize::<T>(text, child, out);
    }
    // If none of the children produced anything, surface this unknown
    if out.len() == before_len {
        out.push(ParsedOrUnknown::Unknown(node.clone()));
    }
}

/// Produce a flat stream of parsed items or unknown structures from the given text.
#[instrument(target = "semantic_query::json_stream", skip(text))]
pub fn deserialize_stream_map<T: DeserializeOwned>(text: &str) -> Vec<ParsedOrUnknown<T>> {
    let mut out = Vec::new();
    let roots = find_json_structures(text);
    for node in &roots {
        descend_deserialize::<T>(text, node, &mut out);
    }
    debug!(target = "semantic_query::json_stream", items = out.len(), "deserialize stream map done");
    out
}

/// Extract all occurrences of `T` from a response string.
///
/// Strategy (in order):
/// - If the entire string parses as `Vec<T>`, return it.
/// - Otherwise, scan for JSON structures. For each structure, try to parse as `Vec<T>`
///   (to support top-level arrays). If any succeeds, extend the result.
/// - Finally, fall back to scanning for individual `T` instances across all structures
///   (using `deserialize_stream_map`) and return them in discovery order.
#[instrument(target = "semantic_query::json_stream", skip(text))]
pub fn extract_all<T: DeserializeOwned>(text: &str) -> Vec<T> {
    // Try direct parse as Vec<T>
    if let Ok(v) = serde_json::from_str::<Vec<T>>(text) {
        return v;
    }

    // Recursively traverse JSON structures, preferring Vec<T> at any node,
    // then T at the same node, otherwise descend to children.
    fn collect_from_node<T: DeserializeOwned>(text: &str, node: &ObjCoords, out: &mut Vec<T>) -> bool {
        let slice_end = node.end + 1;
        let s = &text[node.start..slice_end];
        if let Ok(vs) = serde_json::from_str::<Vec<T>>(s) {
            out.extend(vs);
            return true; // consumed node; skip children
        }
        if let Ok(v) = serde_json::from_str::<T>(s) {
            out.push(v);
            return true; // consumed node; skip children
        }
        // Descend
        for child in &node.children {
            collect_from_node::<T>(text, child, out);
        }
        false
    }

    let mut out: Vec<T> = Vec::new();
    let roots = find_json_structures(text);
    for node in &roots {
        collect_from_node::<T>(text, node, &mut out);
    }
    out
}

/// Spawn a background task that reads from an `AsyncRead` and streams discovered JSON
/// root coordinates as they are closed. Returns an `mpsc::Receiver<ObjCoords>`.
pub fn stream_coords_from_async_read<R>(mut reader: R, buf_size: usize) -> mpsc::Receiver<ObjCoords>
where
    R: AsyncRead + Send + Unpin + 'static,
{
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        tracing::debug!(target = "semantic_query::json_stream", "spawned stream_coords_from_async_read task");
        let mut parser = JsonStreamParser::new();
        let mut accum = String::new();
        let mut buf = vec![0u8; buf_size.max(1024)];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        accum.push_str(s);
                        for node in parser.feed(s) {
                            // Ignore send errors if receiver dropped
                            trace!(target = "semantic_query::json_stream", start = node.start, end = node.end, kind = ?node.kind, "emitting coords");
                            let _ = tx.send(node).await;
                        }
                    } else {
                        // Non-UTF8 bytes; skip for now (JSON is UTF-8)
                        debug!(target = "semantic_query::json_stream", "skipping non-utf8 chunk");
                    }
                }
                Err(e) => { debug!(target = "semantic_query::json_stream", error = %e, "read error"); break },
            }
        }
        tracing::debug!(target = "semantic_query::json_stream", "stream_coords_from_async_read completed");
    });
    rx
}

/// Spawn a background task that reads from an `AsyncRead` and streams either parsed `T`
/// or unknown coordinates as they are found (parent-first). Uses an internal buffer to
/// slice by coordinates.
pub fn stream_deserialized_from_async_read<R, T>(mut reader: R, buf_size: usize) -> mpsc::Receiver<ParsedOrUnknown<T>>
where
    R: AsyncRead + Send + Unpin + 'static,
    T: DeserializeOwned + Send + 'static,
{
    let (tx, rx) = mpsc::channel(64);
    tokio::spawn(async move {
        tracing::debug!(target = "semantic_query::json_stream", "spawned stream_deserialized_from_async_read task");
        let mut parser = JsonStreamParser::new();
        let mut accum = String::new();
        let mut buf = vec![0u8; buf_size.max(1024)];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        let start_len = accum.len();
                        accum.push_str(s);
                        for node in parser.feed(s) {
                            // Attempt parent-first deserialization on each closed root
                            let mut out = Vec::new();
                            descend_deserialize::<T>(&accum, &node, &mut out);
                            for item in out {
                                trace!(target = "semantic_query::json_stream", "emitting parsed/unknown item");
                                let _ = tx.send(item).await;
                            }
                        }
                        let _ = start_len; // quiet unused warning if any
                    } else {
                        // Non-UTF8 bytes; skip for now
                        debug!(target = "semantic_query::json_stream", "skipping non-utf8 chunk");
                    }
                }
                Err(e) => { debug!(target = "semantic_query::json_stream", error = %e, "read error"); break },
            }
        }
        tracing::debug!(target = "semantic_query::json_stream", "stream_deserialized_from_async_read completed");
    });
    rx
}

/// Stream variant (no channel) that yields `ObjCoords` directly via `futures::Stream`.
pub fn stream_coords<R>(mut reader: R, buf_size: usize) -> impl Stream<Item = ObjCoords>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    stream! {
        let mut parser = JsonStreamParser::new();
        let mut buf = vec![0u8; buf_size.max(1024)];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        for node in parser.feed(s) {
                            yield node;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }
}

/// Stream variant (no channel) that yields `ParsedOrUnknown<T>` via `futures::Stream`.
pub fn stream_parsed<R, T>(mut reader: R, buf_size: usize) -> impl Stream<Item = ParsedOrUnknown<T>>
where
    R: AsyncRead + Unpin + Send + 'static,
    T: DeserializeOwned + Send + 'static,
{
    stream! {
        let mut parser = JsonStreamParser::new();
        let mut accum = String::new();
        let mut buf = vec![0u8; buf_size.max(1024)];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        accum.push_str(s);
                        for node in parser.feed(s) {
                            let mut out = Vec::new();
                            descend_deserialize::<T>(&accum, &node, &mut out);
                            for item in out.into_iter() {
                                yield item;
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }
}
