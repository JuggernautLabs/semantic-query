# Semantic Query

Stream-first, schema-aware AI querying. Preserve interleaved text + structured data, parse JSON reliably from messy or streamed outputs, and render in real time.

## Quick Example
run using 
```
cargo run --example readme_demo
```
This demo shows how you can build a quiz engine!
```rust

use serde::{Deserialize};
use schemars::JsonSchema;
use semantic_query::{core::{QueryResolver, RetryConfig}, clients::flexible::FlexibleClient};

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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Claude client (reads ANTHROPIC_API_KEY from environment)
    let client = FlexibleClient::claude();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    // Get 10 science quiz questions
    let quiz: Quiz = resolver.query_with_schema(
        "Create 10 high school science quiz questions with A, B, C, D answers".to_string()
    ).await?;
    
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

## Providers & Setup

- Families: `claude/` (Anthropic, Bedrock), `deepseek/`, `chatgpt/` (OpenAI + Azure OpenAI).
- Env keys (put in `.env`):
  - `ANTHROPIC_API_KEY=...`
  - `DEEPSEEK_API_KEY=...`
  - `OPENAI_API_KEY=...` or `AZURE_OPENAI_ENDPOINT`, `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_DEPLOYMENT`, `AZURE_OPENAI_API_VERSION`.
- Flexible selection: `FlexibleClient::from_type(ClientType::Claude|DeepSeek|ChatGPT)` or default based on which keys exist.

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
- Non-interactive stream parser demo:
  - `cargo run --example semantic_stream_demo`
- DeepSeek agent streaming (requires `DEEPSEEK_API_KEY`):
  - `cargo run --example deepseek_agent_stream_demo`
- JSON structure coords demo:
  - `cargo run --example json_stream_coords_demo`
- Stream parser stress (chunk boundaries):
  - `cargo run --example stream_parser_stress`

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

## Stream-First Parsing

- Structural scanner: Finds balanced JSON objects/arrays in any text, with byte indices and nested children. Works on full strings and incrementally over chunks.
- SemanticItem<T>: an enum preserving order and fidelity:
  - Text(TextContent { text })
  - Data(T)
- Query APIs:
  - `query_with_schema<T>`: appends JSON Schema for T to the prompt.
  - `query_semantic<T>`: returns `Vec<SemanticItem<T>>` from a one-shot response.
  - `query_semantic_stream<T, R: AsyncRead>`: emits `SemanticItem<T>` as stream arrives.

## Streaming Aggregator (SSE)

Use `streaming::stream_sse_aggregated` to render in real time while also chunking text and extracting structured items.

```rust
use semantic_query::streaming::{AggregatedEvent, stream_sse_aggregated};
use futures_util::{StreamExt, pin_mut};

// reader: AsyncRead from any streaming client (e.g., FlexibleClient::stream_raw_reader)
let evs = stream_sse_aggregated::<_, ToolCall>(reader, 8 * 1024);
pin_mut!(evs);
while let Some(ev) = evs.next().await {
    match ev {
        AggregatedEvent::Token(tok) => print!("{}", tok),         // live typing
        AggregatedEvent::TextChunk(s) => println!("\n[agent] {}", s),
        AggregatedEvent::Data(tc) => println!("\n[toolcall] {}", tc.name),
    }
}
```

## Tests

- Pure tests exercising parser and SSE aggregator: `tests/stream_parser_tests.rs`, `tests/sse_aggregator_tests.rs`.
- DeepSeek live tests (ignored by default): `tests/deepseek_live.rs`.

## Linting

- Rustc warnings: `cargo check --all-targets --examples`
- Strict rustc: `RUSTFLAGS='-D warnings -W unused_braces' cargo check --all-targets --examples`
- Clippy (recommended):
  - `cargo clippy --all-targets --all-features -- -W clippy::all -W clippy::nursery -W clippy::pedantic -W rust-2018-idioms -W warnings`
