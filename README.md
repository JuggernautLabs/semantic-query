# Semantic Query

Stream-first, schema-aware AI querying. Extract structured data from LLM responses while preserving explanatory text, with automatic JSON Schema guidance and real-time streaming support.

## Quick Example
run using 
```
cargo run --example readme_demo
```
This demo shows how you can build a quiz engine!
```rust

use serde::Deserialize;
use schemars::JsonSchema;
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::clients::flexible::FlexibleClient;
use anyhow::Result;

#[derive(Debug, Deserialize, JsonSchema)]
struct QuizQuestion {
    /// The main question text to be asked
    pub question: String,
    /// A brief description or context for the question
    pub description: String,
    /// Answer choice A
    pub a: String,
    /// Answer choice B
    pub b: String,
    /// Answer choice C
    pub c: String,
    /// Answer choice D
    pub d: String,
    /// The correct answer (must be exactly one of: A, B, C, or D)
    #[schemars(regex(pattern = r"^[ABCD]$"))]
    pub correct_answer: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Quiz {
    pub questions: Vec<QuizQuestion>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create Claude client (reads ANTHROPIC_API_KEY from environment)
    let client = FlexibleClient::claude();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    // Get 10 science quiz questions (schema guidance is automatic)
    let response = resolver.query::<Quiz>(
        "Create 10 high school science quiz questions with A, B, C, D answers".to_string()
    ).await?;
    
    // Extract the quiz data (new API returns ParsedResponse with mixed content)
    let quiz = response.first_required()?;
    
    // Administer the quiz
    administer_quiz(quiz.questions).await;
    Ok(())
}

async fn administer_quiz(questions: Vec<QuizQuestion>) {
    let mut score = 0;
    let total = questions.len();
    
    for (i, question) in questions.iter().enumerate() {
        println!("\nQuestion {}: {}", i + 1, question.question);
        println!("A) {}", question.a);
        println!("B) {}", question.b);
        println!("C) {}", question.c);
        println!("D) {}", question.d);
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let answer = input.trim().to_uppercase();
        
        if answer == question.correct_answer.to_uppercase() {
            score += 1;
        }
    }
    
    println!("\n🎯 Quiz Complete! Final Score: {}/{} ({}%)", 
             score, total, (score * 100) / total);
}
```

**Setup**: Add `ANTHROPIC_API_KEY=your_key_here` to `.env` file.

## Modern API: Mixed Content Support

The new API recognizes that LLMs naturally produce mixed content - explanatory text alongside structured data. Instead of forcing everything to be JSON, we preserve both:

```rust
// Query with automatic schema guidance
let response = resolver.query::<Analysis>("Analyze this code for issues").await?;

// Access different parts of the response
let all_analyses = response.data_only();        // Vec<&Analysis> - all structured data
let full_text = response.text_content();        // String - complete text including JSON
let first = response.first_required()?;         // Analysis - first item or error

// Iterate through mixed content preserving order
for item in &response.items {
    match item {
        ResponseItem::Text(text) => println!("Explanation: {}", text.text),
        ResponseItem::Data { data, original_text } => {
            println!("Found issue: {}", data.issue);
            println!("Original JSON: {}", original_text);
        }
    }
}
```

### Real-Time Streaming

Stream responses token-by-token while automatically extracting structured data:

```rust
use futures_util::StreamExt;
use semantic_query::streaming::StreamItem;

let mut stream = resolver.stream_query::<ToolCall>("Help me debug this").await?;
while let Some(item) = stream.next().await {
    match item? {
        StreamItem::Token(tok) => print!("{}", tok),  // Real-time text
        StreamItem::Text(text) => {                    // Completed text chunk
            println!("\n[Assistant] {}", text.text);
        }
        StreamItem::Data(tool) => {                    // Structured data found
            println!("\n[Tool Call] {}: {:?}", tool.name, tool.args);
        }
    }
}
```

## Providers & Setup

- Families: `claude/` (Anthropic, Bedrock), `deepseek/`, `chatgpt/` (OpenAI + Azure OpenAI).
- Env keys (put in `.env`):
  - `ANTHROPIC_API_KEY=...`
  - `DEEPSEEK_API_KEY=...`
  - `OPENAI_API_KEY=...` or `AZURE_OPENAI_ENDPOINT`, `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_DEPLOYMENT`, `AZURE_OPENAI_API_VERSION`.
- Flexible selection: `FlexibleClient::from_type(ClientType::Claude|DeepSeek|ChatGPT)` or default based on which keys exist.

### Bedrock (Claude) Support

- Bedrock is available for Claude only, and is completely feature-gated.
- To enable Bedrock via AWS SDK:
  - Enable features: `aws-bedrock-sdk,bedrock,anthropic`
  - Example build: `cargo run --example bedrock_stream_demo --features aws-bedrock-sdk,bedrock,anthropic`
  - Provide AWS credentials and a region (e.g., `AWS_REGION=us-east-1`).
- Notes:
  - When disabled, Bedrock code is not compiled or exported — it’s impossible to reference it.
  - Streaming uses Bedrock Runtime’s `InvokeModelWithResponseStream` and falls back to one-shot `InvokeModel` if streaming is not supported by the selected model.

## Logging via .env

This project uses `tracing` for logs and reads env from `.env` (via `dotenvy`). Set `RUST_LOG` in `.env` to control verbosity without passing flags:

Examples:
- Verbose parser + resolver logs:
  ```env
  RUST_LOG=semantic_query::json_stream=trace,semantic_query::resolver=debug
  ```
- Parser-only logs:
  ```env
  RUST_LOG=semantic_query::json_stream=debug
  ```
- Resolver-only logs:
  ```env
  RUST_LOG=semantic_query::resolver=info
  ```

Then run any example or test normally and logs will appear.

Examples:
- Main demo with quiz generation:
  - `cargo run --example readme_demo`
- Streaming demo with real-time output:
  - `cargo run --example readme_demo_streaming`
- Mixed content demo showing V2 improvements:
  - `cargo run --example resolver_v2_demo`
- JSON structure coordinates demo:
  - `cargo run --example json_stream_coords_demo`
- Schema validation demo:
  - `cargo run --example schema_demo`
- Benchmark tool (tests all providers):
  - `cargo run --bin benchmark`

DeepSeek live tests (ignored by default; requires network + key):
```
cargo test --test deepseek_live -- --ignored --nocapture
```
Set `DEEPSEEK_API_KEY` in your `.env` (FlexibleClient loads it automatically).

## Schema-Aware Prompt Generation

The doc comments and constraints in your structs are automatically converted to JSON schema and included in the AI prompt. Here's what the actual prompt looks like for the QuizQuestion struct:

```
Create 10 high school science quiz questions with A, B, C, D answers

You are tasked with generating a value satisfying a schema. First I will give you an example exchange then I will provide the schema of interest
Example Schema:
{
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "age": {"type": "integer", "minimum": 0},
        "email": {"type": "string"},
        "isActive": {"type": "boolean"},
        "hobbies": {"type": "array", "items": {"type": "string"}}
    },
    "required": ["name", "age", "email", "isActive"]
}
Example response:
{"name": "Alice Smith", "age": 28, "email": "alice@example.com", "isActive": true, "hobbies": ["reading", "cooking"]}
Please provide a response matching this schema
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Quiz",
  "type": "object",
  "required": [
    "questions"
  ],
  "properties": {
    "questions": {
      "type": "array",
      "items": {
        "$ref": "#/definitions/QuizQuestion"
      }
    }
  },
  "definitions": {
    "QuizQuestion": {
      "title": "QuizQuestion",
      "type": "object",
      "required": [
        "question",
        "description",
        "a",
        "b", 
        "c",
        "d",
        "correct_answer"
      ],
      "properties": {
        "question": {
          "description": "The main question text to be asked",
          "type": "string"
        },
        "description": {
          "description": "A brief description or context for the question", 
          "type": "string"
        },
        "a": {
          "description": "Answer choice A",
          "type": "string"
        },
        "b": {
          "description": "Answer choice B", 
          "type": "string"
        },
        "c": {
          "description": "Answer choice C",
          "type": "string"
        },
        "d": {
          "description": "Answer choice D",
          "type": "string"
        },
        "correct_answer": {
          "description": "The correct answer (must be exactly one of: A, B, C, or D)",
          "type": "string",
          "pattern": "^[ABCD]$"
        }
      }
    }
  }
}
```
```

This schema ensures the AI understands exactly what each field represents and enforces the constraint that `correct_answer` must be exactly A, B, C, or D.

## Core Features

### Stream-First JSON Parsing

- **Structural scanner**: Finds balanced JSON objects/arrays in any text, with byte indices and nested children. Works on full strings and incrementally over chunks.
- **Mixed content preservation**: LLM responses often mix explanatory text with JSON - we preserve both in order
- **Robust extraction**: Handles malformed JSON, partial objects, and nested structures

### Type-Safe APIs

- **`query<T>`**: Main API - automatically adds JSON Schema guidance and returns `ParsedResponse<T>`
- **`query_mixed<T>`**: Raw mixed content without schema guidance  
- **`stream_query<T>`**: Real-time streaming with automatic JSON extraction
- **`first_required()`**: Clean error handling for single-item extraction

### Response Types

- **`ParsedResponse<T>`**: Contains ordered items (text + data) from the response
- **`ResponseItem<T>`**: Either `Text(content)` or `Data { data: T, original_text }`
- **`StreamItem<T>`**: Streaming variant with `Token`, `Text`, and `Data`

### Streaming Providers

- Claude (Anthropic): streaming enabled.
- Claude (Bedrock): streaming enabled when built with `aws-bedrock-sdk`.
- DeepSeek: streaming enabled.
- ChatGPT (OpenAI/Azure): streaming enabled.

## Migration from Legacy API

The old single-item APIs are deprecated. Here's how to migrate:

```rust
// Old API (deprecated)
let result: T = resolver.query_with_schema::<T>(prompt).await?;

// New API - Option 1: Simple migration with first_required()
let result: T = resolver.query::<T>(prompt).await?.first_required()?;

// New API - Option 2: Handle multiple results
let response = resolver.query::<T>(prompt).await?;
for item in response.data_only() {
    process_item(item);
}

// New API - Option 3: Access mixed content
let response = resolver.query::<T>(prompt).await?;
println!("Full explanation: {}", response.text_content());
println!("Found {} data items", response.data_count());
```

## Tests

- Pure tests exercising parser and SSE aggregator: `tests/stream_parser_tests.rs`, `tests/sse_aggregator_tests.rs`.
- DeepSeek live tests (ignored by default): `tests/deepseek_live.rs`.

## Linting

- Rustc warnings: `cargo check --all-targets --examples`
- Strict rustc: `RUSTFLAGS='-D warnings -W unused_braces' cargo check --all-targets --examples`
- Clippy (recommended):
  - `cargo clippy --all-targets --all-features -- -W clippy::all -W clippy::nursery -W clippy::pedantic -W rust-2018-idioms -W warnings`

## Models

- ChatGPT family includes `OpenAIModel::Gpt5` → `"gpt-5"` (in addition to `gpt-4o`, `gpt-4o-mini`, etc.).
