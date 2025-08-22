#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use semantic_query::clients::flexible::FlexibleClient;
use semantic_query::{core::{QueryResolver, RetryConfig}};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct Quiz {
    pub questions: Vec<QuizQuestion>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env first so RUST_LOG in .env is seen
    let _ = dotenvy::dotenv();
    // Initialize tracing from RUST_LOG if provided
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    // Create client (env handled by FlexibleClient)
    let client = FlexibleClient::deepseek();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    // Get 10 science quiz questions
    let result = resolver.query::<Quiz>(
        "Create 1 high school science quiz questions with A, B, C, D answers".to_string()
    ).await?;
    
    let quiz = result.first().ok_or("No quiz data found in response")?.clone();
    
    // Show mixed content capabilities of V2
    println!("🎯 Generated Quiz with Mixed Content:");
    println!("   Questions found: {}", quiz.questions.len());
    println!("   Total response length: {} characters", result.text_content().len());
    println!("   Data items: {}", result.data_count());
    if result.text_content().len() > 300 {
        println!("   Context preview: {}...", result.text_content().chars().take(200).collect::<String>());
    } else {
        println!("   Full context: {}", result.text_content());
    }
    
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
