use schemars::{schema_for, JsonSchema};
use semantic_query::core::{QueryResolver, RetryConfig};
use serde::Serialize;
use serde_json::Value;
use serde::de::DeserializeOwned;
use bytes::Bytes;
use futures_core::Stream;
use futures_util::StreamExt;
use async_stream::stream;

/// Response kinds supported by this prompt kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind {
    /// Interleave free-form text with structured items (Vec<StreamItem<T>>)
    SemanticInterleave,
}

/// Guidance constraints to shape the model’s response.
#[derive(Debug, Clone)]
pub struct Guidance {
    pub allow_prose: bool,
    pub allow_code_fences: bool,
    pub min_tool_calls: Option<u8>,
    pub streaming: bool,
    pub require_wrapped_semantic_items: bool,
}

impl Default for Guidance {
    fn default() -> Self {
        Self {
            allow_prose: true,
            allow_code_fences: false,
            min_tool_calls: None,
            streaming: true,
            require_wrapped_semantic_items: true,
        }
    }
}

/// Provider-specific hints (reserved for future normalization knobs).
#[derive(Debug, Clone, Default)]
pub struct ProviderHints {}

/// Prompt specification for first-class prompts.
#[derive(Debug, Clone)]
pub struct PromptSpec<T> {
    pub kind: ResponseKind,
    pub system: String,
    pub task: String,
    pub guidance: Guidance,
    pub provider_hints: ProviderHints,
    pub version: String,
    pub schema_json: String,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: JsonSchema> PromptSpec<T> {
    /// Build a default semantic interleave v1 spec.
    pub fn semantic_interleave_v1(system: impl Into<String>, task: impl Into<String>) -> Self {
        let schema = schema_for!(Vec<semantic_query::semantic::StreamItem<T>>);
        let schema_json = serde_json::to_string_pretty(&schema).unwrap_or_else(|_| "{}".to_string());
        Self {
            kind: ResponseKind::SemanticInterleave,
            system: system.into(),
            task: task.into(),
            guidance: Guidance::default(),
            provider_hints: ProviderHints::default(),
            version: "semantic_interleave_v1".to_string(),
            schema_json,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Render the prompt spec to a single prompt string.
/// For now, we render a unified text block; provider adapters can be added later.
pub fn render_prompt<T>(spec: &PromptSpec<T>) -> String
where
    T: JsonSchema,
{
    // Guidance wording derived from constraints.
    let mut guidance_lines: Vec<String> = Vec::new();
    guidance_lines.push("Respond as an assistant that interleaves plain text with tool calls.".to_string());
    if spec.guidance.require_wrapped_semantic_items {
        guidance_lines.push("Include a JSON array of items using the provided schema. You may include other text before or after; ensure the JSON array is valid and intact.".to_string());
        guidance_lines.push("Each item must be one of: (1) Text: {\"kind\":\"Text\",\"content\":{\"text\":\"...\"}} (2) Data: {\"kind\":\"Data\",\"content\": <object matching the provided schema>}".to_string());
    }
    if !spec.guidance.allow_code_fences {
        guidance_lines.push("Do not wrap JSON in code fences.".to_string());
    }
    if let Some(n) = spec.guidance.min_tool_calls {
        guidance_lines.push(format!("Provide at least {n} tool call(s) that make sense together."));
    }

    format!(
        "[prompt_id: {version}]\nSystem:\n{system}\n\nTask:\n{task}\n\nGuidance:\n- {guidance}\n\nSchema (for the JSON array of items):\n```json\n{schema}\n```\n",
        version = spec.version,
        system = spec.system,
        task = spec.task,
        guidance = guidance_lines.join("\n- "),
        schema = spec.schema_json
    )
}

/// Streaming APIs that keep `T` at call time and use the underlying client's streaming.
impl<T> PromptSpec<T>
where
    T: DeserializeOwned + JsonSchema + Send + 'static,
{
    /// Stream aggregated SSE events (Token/TextChunk/Data) using this prompt and a client.
    pub fn stream_events_with_client(
        &self,
        client: impl semantic_query::core::LowLevelClient + 'static,
    ) -> Result<
        std::pin::Pin<Box<dyn Stream<Item = Result<semantic_query::streaming::AggregatedEvent<T>, semantic_query::error::QueryResolverError>> + Send>>,
        semantic_query::error::QueryResolverError,
    > {
       let qr = QueryResolver::new(client, RetryConfig::default());
       qr.query_semantic_stream(reader, buf_size)

    }

    /// Stream semantic items (Text/Data(T)) using this prompt and a client.
    pub fn stream_semantic_with_client(
        &self,
        client: impl semantic_query::core::LowLevelClient + 'static,
    ) -> Result<
        std::pin::Pin<Box<dyn Stream<Item = Result<semantic_query::semantic::StreamItem<T>, semantic_query::error::QueryResolverError>> + Send>>,
        semantic_query::error::QueryResolverError,
    > {
        let prompt = render_prompt(self);
        if let Some(byte_stream) = client.stream_raw(prompt) {
            let s = semantic_query::semantic::stream_semantic_from_sse_bytes::<T>(Box::pin(byte_stream));
            return Ok(Box::pin(s));
        }

        // Fallback for non-streaming clients: one-shot -> stream of items
        let prompt2 = render_prompt(self);
        let s = stream! {
            match client.ask_raw(prompt2).await {
                Ok(raw) => {
                    let items = semantic_query::semantic::build_semantic_stream::<T>(&raw);
                    for item in items {
                        yield Ok(item);
                    }
                }
                Err(e) => { yield Err(semantic_query::error::QueryResolverError::Ai(e)); }
            }
        };
        Ok(Box::pin(s))
    }
}

// Re-export common semantic_query items for downstream convenience
pub mod prelude {
    pub use semantic_query::semantic::{StreamItem, TextContent};
}
