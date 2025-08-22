#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use semantic_query::clients::flexible::FlexibleClient;
use semantic_query::{core::{QueryResolver, RetryConfig}, streaming::StreamItem};
use futures_util::StreamExt;

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
    
    println!("🎮 Streaming Quiz Generator");
    println!("==========================");
    println!("Generating quiz questions in real-time...\n");
    
    // Stream quiz generation
    let mut stream = resolver.stream_query::<Quiz>(
        "Create 3 high school science quiz questions with A, B, C, D answers. Think step by step and explain your reasoning.".to_string()
    ).await?;
    
    let mut quiz_data: Option<Quiz> = None;
    let mut text_chunks = Vec::new();
    
    while let Some(item_result) = stream.next().await {
        match item_result? {
            StreamItem::Token(token) => {
                print!("{}", token);
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }
            StreamItem::Text(text) => {
                println!("\n📝 [Text Chunk]: {}", text.text);
                text_chunks.push(text.text);
            }
            StreamItem::Data(quiz) => {
                println!("\n🎯 [Quiz Generated]!");
                println!("   Found {} questions", quiz.questions.len());
                quiz_data = Some(quiz);
            }
        }
    }
    
    println!("\n{}","=".repeat(50));
    
    if let Some(quiz) = quiz_data {
        println!("\n✅ Quiz Generation Complete!");
        println!("   Questions: {}", quiz.questions.len());
        println!("   Text chunks captured: {}", text_chunks.len());
        println!("   Total explanatory text: {} chars", text_chunks.join(" ").len());
        
        // Show first question as preview
        if !quiz.questions.is_empty() {
            let q = &quiz.questions[0];
            println!("\n📋 Sample Question:");
            println!("   Q: {}", q.question);
            println!("   A) {} | B) {}", q.a, q.b);
            println!("   C) {} | D) {}", q.c, q.d);
            println!("   Correct: {}", q.correct_answer);
        }
        
        // Administer the quiz
        println!("\n🎯 Starting Quiz!");
        administer_quiz(quiz.questions).await;
    } else {
        println!("❌ No quiz data was generated in the stream");
    }
    
    Ok(())
}

async fn administer_quiz(questions: Vec<QuizQuestion>) {
    let mut score = 0;
    let total = questions.len();
    
    for (i, question) in questions.iter().enumerate() {
        println!("\nQuestion {}: {}", i + 1, question.question);
        if !question.description.is_empty() {
            println!("Context: {}", question.description);
        }
        println!("A) {}", question.a);
        println!("B) {}", question.b);
        println!("C) {}", question.c);
        println!("D) {}", question.d);
        
        print!("\nYour answer (A/B/C/D): ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let answer = input.trim().to_uppercase();
        
        if answer == question.correct_answer.to_uppercase() {
            println!("✅ Correct!");
            score += 1;
        } else {
            println!("❌ Wrong! The correct answer was: {}", question.correct_answer);
        }
    }
    
    println!("\n🎯 Quiz Complete! Final Score: {}/{} ({:.1}%)", 
             score, total, (score as f64 / total as f64) * 100.0);
    
    if score == total {
        println!("🏆 Perfect score! Excellent work!");
    } else if score as f64 / total as f64 > 0.7 {
        println!("👏 Great job! You know your science!");
    } else {
        println!("📚 Keep studying - you'll get it next time!");
    }
}