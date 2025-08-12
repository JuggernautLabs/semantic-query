use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::json_utils::{find_json_structures, deserialize_stream_map, ParsedOrUnknown};
use tracing::{debug, instrument};

/// Represents a piece of unstructured text content returned by the model.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct TextContent {
    /// Plain text content. Downstream systems can render or log this.
    pub text: String,
}

/// A semantic item in the model's response stream: either text or typed data `T`.
///
/// Use this as `Vec<SemanticItem<T>>` to preserve order while giving
/// downstream systems full fidelity of both text and structured output.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "content")]
pub enum SemanticItem<T>
where
    T: JsonSchema,
{
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
