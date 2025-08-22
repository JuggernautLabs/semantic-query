# Extreme TODO 2 — First-class Prompt System

Make prompts explicit, versioned, linted, and provider-aware. Enforce use of the semantic schema only and eliminate accidental contradictions through typed specs and renderers.

## Goals

- Replace ad-hoc string augmentation with a typed PromptSpec + renderer pipeline.
- Use only the semantic schema (Vec<StreamItem<T>>); remove data-only prompting paths.
- Provider-specific rendering without leaking provider quirks into call sites.
- Lint prompts for contradictions (e.g., “JSON only” vs. allow prose).
- Snapshot prompts for reviewable diffs and reproducibility.

## Non-Goals

- Reinvent modeling/clients. This focuses on prompt surfacing, not transport.
- Add a templating DSL. Keep templates minimal and data-driven.

## Deliverables

- `src/prompts/` module with:
  - `spec.rs`: `PromptSpec<T>`, `Guidance`, `ProviderHints`, builder.
  - `render.rs`: `PromptRenderer`, provider adapters (Anthropic/OpenAI/DeepSeek).
  - `lints.rs`: compile-time/runtime lints for PromptSpec.
  - `ids.rs`: stable hash → prompt ID/checksum.
  - `templates/semantic_interleave_v1.txt`: minimal text template.
- Public API:
  - `PromptSpec::semantic_interleave_v1::<T>()` convenience.
  - `QueryResolver::stream_semantic_with_spec<T>(spec)`.
  - Keep `stream_semantic<T>` as a thin wrapper around default spec.
- Examples updated to pass `PromptSpec` (at least one streaming demo).
- Snapshot tests for rendered prompts (under `prompts/snapshots/`).

## Architecture Sketch

```rust
// Response kinds: we only support semantic interleave (text + data)
pub enum ResponseKind { SemanticInterleave }

pub struct Guidance {
    pub allow_prose: bool,
    pub allow_code_fences: bool,
    pub min_tool_calls: Option<u8>,
    pub streaming: bool,
    pub require_wrapped_semantic_items: bool, // always true by default
}

pub struct ProviderHints {
    // future: style toggles per provider if necessary
}

pub struct PromptSpec<T> {
    pub kind: ResponseKind,
    pub system: String,
    pub task: String,
    pub guidance: Guidance,
    pub provider_hints: ProviderHints,
    pub version: String, // e.g., "semantic_interleave_v1"
    pub schema_json: String, // JSON Schema for Vec<StreamItem<T>>
    _phantom: std::marker::PhantomData<T>,
}

impl<T: schemars::JsonSchema> PromptSpec<T> {
    pub fn semantic_interleave_v1(system: impl Into<String>, task: impl Into<String>) -> Self { /* build with defaults */ }
}

pub trait PromptRenderer {
    fn render<T>(&self, spec: &PromptSpec<T>) -> ProviderMessages
    where T: schemars::JsonSchema;
}

// Query integration
impl<C: LowLevelClient> QueryResolver<C> {
    pub async fn stream_semantic_with_spec<T>(&self, spec: PromptSpec<T>) -> Result<Pin<Box<dyn Stream<Item = Result<StreamItem<T>, QueryResolverError>> + Send>>, QueryResolverError>
    where T: DeserializeOwned + JsonSchema + Send + 'static {
        let messages = prompts::render_for_client(&spec, self.client());
        // call client.stream_raw with rendered prompt/messages, then parse:
        Ok(Box::pin(crate::semantic::stream_semantic_from_sse_bytes::<T>(
            self.client().stream_raw(messages.into_string()).ok_or_else(/* … */)?
        )))
    }
}
```

## Lint Rules (examples)

- Error: `guidance.allow_prose == true` but template text contains “JSON only”.
- Error: `require_wrapped_semantic_items == true` but schema is not `Vec<StreamItem<T>>`.
- Warn: `min_tool_calls.is_some()` but guidance text omits tool-call instructions.
- Error: examples include code fences while `allow_code_fences == false`.

## Provider Rendering

- Anthropic: system → system message; task+guidance+schema → user message.
- OpenAI/Azure/DeepSeek: system role + user role messages; include schema block verbatim.
- All providers: include prompt ID/checksum in a non-instructional metadata line for tracing.

## Implementation Plan

1) Scaffold `src/prompts/{spec,render,lints,ids}.rs` and template for `semantic_interleave_v1`.
2) Add `QueryResolver::stream_semantic_with_spec` and make `stream_semantic` call it with default spec.
3) Add snapshot tests for rendered prompt (golden file under `prompts/snapshots/`).
4) Update one example (`simple_agent_stream_demo`) to build/use `PromptSpec`.
5) Add lint pass in debug builds; fail-fast on lint errors in tests.
6) Deprecate data-only augmentation behind a feature (e.g., `legacy-data-schema`).

## Acceptance Criteria

- Examples run and stream `Text/Data` without requiring “JSON only” phrasing.
- Lints catch contradictions; CI fails on lint errors in prompts.
- Snapshots show stable, reviewable prompt changes.
- Only semantic schema appears in prompts (no standalone `T` schema).

## Risks / Open Questions

- Provider drift: may need small per-provider guidance text variants.
- Back-compat: keep `stream_semantic` for now; remove legacy path in a future major release.
- Prompt ID placement: ensure it never affects provider behavior (use comments or metadata if supported).

