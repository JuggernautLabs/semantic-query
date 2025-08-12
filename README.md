# Semantic Query

AI-powered schema validation with automatic JSON generation for type-safe responses.

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
    let quiz: Quiz = resolver.query(
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

Non-interactive example (safe to run):
```
cargo run --example semantic_stream_demo
```

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
