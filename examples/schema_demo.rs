use semantic_query::{clients::MockClient, core::RetryConfig, QueryResolverV2};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    
    let(client, handle) = MockClient::new();
    let resolver = QueryResolverV2::new(client, RetryConfig::default());
    // Add realistic analysis response with mixed content
    handle.add_response(semantic_query::clients::MockResponse::Success(r#"
I'll analyze this code for security vulnerabilities. Let me examine it systematically.

After reviewing the code, here's my security analysis:

{
  "confidence": 0.85,
  "finding": "Code contains potential SQL injection vulnerability",
  "evidence": [
    "String concatenation used for SQL queries",
    "No input validation on user parameters",
    "Missing prepared statements"
  ],
  "severity": "high",
  "metadata": {
    "scan_time": "2025-01-15T10:30:00Z",
    "tool_version": "security-scanner-v2.1"
  }
}

This vulnerability should be addressed by using parameterized queries and input validation.
"#.trim().to_string()));

    // Add realistic code review response
    handle.add_response(semantic_query::clients::MockResponse::Success(r#"
I'll review this Rust function for code quality. Let me analyze the implementation.

Here's my comprehensive code review:

{
  "quality_score": 7,
  "issues": [
    {
      "issue_type": "style",
      "severity": "low",
      "description": "Function name should use snake_case",
      "line_number": 15,
      "suggested_fix": "Rename function to follow Rust naming conventions"
    },
    {
      "issue_type": "performance",
      "severity": "medium", 
      "description": "Unnecessary allocation in loop",
      "line_number": 23,
      "suggested_fix": "Use iterator methods instead of collecting to Vec"
    }
  ],
  "strengths": [
    "Good error handling with Result types",
    "Proper use of ownership and borrowing",
    "Clear variable names"
  ],
  "recommendations": [
    "Add unit tests for edge cases",
    "Consider using more specific error types",
    "Add documentation comments"
  ]
}

Overall, this is solid Rust code with room for minor improvements.
"#.trim().to_string()));
    // Example 1: Analysis Result
    println!("=== Example 1: AnalysisResult Schema ===");
    let analysis_prompt = "Analyze this code for security vulnerabilities".to_string();
    
    match resolver.query::<AnalysisResult>(analysis_prompt).await {
        Ok(result) => {
            if let Some(analysis) = result.first() {
                println!("✅ Security Analysis Found:");
                println!("   Confidence: {:.1}%", analysis.confidence * 100.0);
                println!("   Finding: {}", analysis.finding);
                println!("   Severity: {}", analysis.severity);
                println!("   Evidence count: {}", analysis.evidence.len());
                
                println!("\n📝 Full response with context:");
                println!("   {}", result.text_content().chars().take(200).collect::<String>());
                if result.text_content().len() > 200 {
                    println!("   ... (truncated)");
                }
            } else {
                println!("❌ No analysis data found");
            }
        }
        Err(e) => println!("❌ Query failed: {}", e),
    }
    
    println!("\n=== Example 2: CodeReview Schema ===");
    let review_prompt = "Review this Rust function for code quality".to_string();
    
    match resolver.query::<CodeReview>(review_prompt).await {
        Ok(result) => {
            if let Some(review) = result.first() {
                println!("✅ Code Review Found:");
                println!("   Quality Score: {}/10", review.quality_score);
                println!("   Issues Found: {}", review.issues.len());
                println!("   Strengths: {}", review.strengths.len());
                println!("   Recommendations: {}", review.recommendations.len());
                
                if !review.issues.is_empty() {
                    println!("\n🔍 Top Issue:");
                    let issue = &review.issues[0];
                    println!("   {} ({}): {}", issue.issue_type, issue.severity, issue.description);
                }
                
                println!("\n📝 V2 preserves all explanatory text too!");
            } else {
                println!("❌ No review data found");
            }
        }
        Err(e) => println!("❌ Query failed: {}", e),
    }
    
    println!("\n=== V2 Benefits ===");
    println!("• QueryResolverV2 automatically includes JSON schema in prompts");
    println!("• Preserves both structured data and explanatory text");
    println!("• Handles multiple data items in responses");
    println!("• Better error reporting with context");
    
    Ok(())
}
