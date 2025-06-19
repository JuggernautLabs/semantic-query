use semantic_query::{core::{QueryResolver, RetryConfig}, clients::mock::MockVoid};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisResult {
    /// The confidence score from 0.0 to 1.0
    pub confidence: f64,
    /// The main finding or conclusion
    pub finding: String,
    /// List of supporting evidence
    pub evidence: Vec<String>,
    /// Severity level: low, medium, high, critical
    pub severity: String,
    /// Additional metadata
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CodeReview {
    /// Overall quality score from 1-10
    pub quality_score: u8,
    /// List of issues found
    pub issues: Vec<Issue>,
    /// List of positive aspects
    pub strengths: Vec<String>,
    /// Recommended next steps
    pub recommendations: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Issue {
    /// Type of issue: bug, performance, style, security
    pub issue_type: String,
    /// Severity: low, medium, high, critical
    pub severity: String,
    /// Description of the issue
    pub description: String,
    /// Line number where issue occurs (if applicable)
    pub line_number: Option<u32>,
    /// Suggested fix
    pub suggested_fix: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let client = MockVoid;
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    // Example 1: Analysis Result
    println!("=== Example 1: AnalysisResult Schema ===");
    let analysis_prompt = "Analyze this code for security vulnerabilities".to_string();
    
    // This would normally work with a real AI client, but MockVoid just returns "{}"
    // The important part is seeing how the schema gets generated and appended
    match resolver.query::<AnalysisResult>(analysis_prompt).await {
        Ok(result) => println!("Analysis result: {:?}", result),
        Err(e) => println!("Expected error with MockVoid: {}", e),
    }
    
    println!("\n=== Example 2: CodeReview Schema ===");
    let review_prompt = "Review this Rust function for code quality".to_string();
    
    match resolver.query::<CodeReview>(review_prompt).await {
        Ok(result) => println!("Review result: {:?}", result),
        Err(e) => println!("Expected error with MockVoid: {}", e),
    }
    
    // Demonstrate the schema generation directly
    println!("\n=== Generated Schema Examples ===");
    
    let analysis_schema_prompt = resolver.augment_prompt_with_schema::<AnalysisResult>(
        "Analyze this code".to_string()
    );
    println!("AnalysisResult schema-augmented prompt:");
    println!("{}", analysis_schema_prompt);
    
    println!("\n{}", "=".repeat(80));
    
    let review_schema_prompt = resolver.augment_prompt_with_schema::<CodeReview>(
        "Review this code".to_string()
    );
    println!("CodeReview schema-augmented prompt:");
    println!("{}", review_schema_prompt);
    
    Ok(())
}