# Strong Query

AI-powered schema validation with automatic JSON generation for type-safe responses.

## Quick Example
run using 
```
cargo run --example readme_demo
```
```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use semanic_query::client::{QueryResolver, RetryConfig};
use semanic_query::claude::ClaudeClient;

#[derive(Debug, Deserialize, JsonSchema)]
struct QuizQuestion {
    pub question: String,
    pub a: String,
    pub b: String,
    pub c: String,
    pub d: String,
    pub correct_answer: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Quiz {
    pub questions: Vec<QuizQuestion>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create Claude client (reads ANTHROPIC_API_KEY from environment)
    let client = ClaudeClient::new()?;
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