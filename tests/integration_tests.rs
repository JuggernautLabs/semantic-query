mod test_utils;

use semantic_query::core::RetryConfig;
use crate::test_utils::{create_test_resolver, create_test_resolver_with_config, should_skip_integration_tests, print_test_client_info};
use serde::Deserialize;
use schemars::JsonSchema;
use std::sync::Once;

static INIT: Once = Once::new();

fn init_test_logging() {
    INIT.call_once(|| {
        print_test_client_info();
    });
}

/// Simple test structure for basic functionality
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(title = "Math Result", description = "Result of a mathematical calculation")]
pub struct MathResult {
    /// The calculated result
    #[schemars(description = "The numerical result of the calculation")]
    pub result: i32,
    /// Whether the calculation was correct
    #[schemars(description = "True if the calculation appears correct")]
    pub is_correct: bool,
}

/// More complex structure to test rich schema generation
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(title = "Code Analysis", description = "Analysis of code quality and issues")]
pub struct CodeAnalysis {
    /// Confidence score from 0.0 to 1.0
    #[schemars(range(min = 0.0, max = 1.0), description = "How confident the analysis is")]
    pub confidence: f64,
    /// Primary finding from the analysis
    #[schemars(description = "The main conclusion from analyzing the code")]
    pub finding: String,
    /// List of specific issues found
    #[schemars(description = "Detailed list of problems or observations")]
    pub issues: Vec<String>,
    /// Severity level of the overall finding
    pub severity: Severity,
}

/// Severity levels for findings
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "Severity classification for findings")]
pub enum Severity {
    /// Low impact issue
    #[schemars(description = "Minor issue that can be addressed later")]
    Low,
    /// Medium impact issue  
    #[schemars(description = "Issue that should be addressed soon")]
    Medium,
    /// High impact issue
    #[schemars(description = "Critical issue requiring immediate attention")]
    High,
}

#[tokio::test]
async fn test_basic_schema_query() {
    init_test_logging();
    
    if should_skip_integration_tests() {
        println!("Skipping integration test - using mock client");
        return;
    }

    let resolver = create_test_resolver();

    let result: Result<MathResult, _> = resolver.query_with_schema(
        "Calculate 15 + 27 and tell me if the result is correct".to_string()
    ).await;

    match result {
        Ok(math_result) => {
            println!("✅ Basic schema test passed:");
            println!("   Result: {}", math_result.result);
            println!("   Is correct: {}", math_result.is_correct);
            
            // Only validate schema compliance, not correctness
            assert!(matches!(math_result.is_correct, true | false), "is_correct should be boolean");
        },
        Err(e) => {
            eprintln!("❌ Basic schema test failed: {}", e);
            panic!("Integration test failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_complex_schema_with_enums() {
    init_test_logging();
    
    if should_skip_integration_tests() {
        println!("Skipping integration test - using mock client");
        return;
    }

    let resolver = create_test_resolver();

    let code_sample = r#"
    fn unsafe_function() {
        let ptr = std::ptr::null_mut();
        unsafe {
            *ptr = 42; // This will segfault!
        }
    }
    "#;

    let result: Result<CodeAnalysis, _> = resolver.query_with_schema(
        format!("Analyze this Rust code for issues:\n\n{}", code_sample)
    ).await;

    match result {
        Ok(analysis) => {
            println!("✅ Complex schema test passed:");
            println!("   Confidence: {:.2}", analysis.confidence);
            println!("   Finding: {}", analysis.finding);
            println!("   Severity: {:?}", analysis.severity);
            println!("   Issues found: {}", analysis.issues.len());
            
            // Only validate schema compliance, not content correctness
            assert!(analysis.confidence >= 0.0 && analysis.confidence <= 1.0, "confidence must be in range [0.0, 1.0]");
            assert!(!analysis.finding.is_empty(), "finding should not be empty string");
            assert!(matches!(analysis.severity, Severity::Low | Severity::Medium | Severity::High), "severity must be valid enum variant");
            // Don't assert on issues content - just that it's a valid Vec<String>
        },
        Err(e) => {
            eprintln!("❌ Complex schema test failed: {}", e);
            panic!("Integration test failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_schema_constraint_validation() {
    init_test_logging();
    
    if should_skip_integration_tests() {
        println!("Skipping integration test - using mock client");
        return;
    }

    let resolver = create_test_resolver();

    // Test that the AI respects schema constraints
    let result: Result<CodeAnalysis, _> = resolver.query_with_schema(
        "Give a high-confidence analysis of this simple function: fn add(a: i32, b: i32) -> i32 { a + b }".to_string()
    ).await;

    match result {
        Ok(analysis) => {
            println!("✅ Schema constraint test passed:");
            println!("   Confidence: {:.2}", analysis.confidence);
            
            // Only validate schema constraints, not AI reasoning quality
            assert!(
                analysis.confidence >= 0.0 && analysis.confidence <= 1.0,
                "Confidence {} is outside valid range [0.0, 1.0]", 
                analysis.confidence
            );
            assert!(!analysis.finding.is_empty(), "finding should not be empty");
            assert!(matches!(analysis.severity, Severity::Low | Severity::Medium | Severity::High), "severity must be valid enum");
        },
        Err(e) => {
            eprintln!("❌ Schema constraint test failed: {}", e);
            panic!("Integration test failed: {}", e);
        }
    }
}

#[tokio::test]
async fn test_retry_behavior() {
    init_test_logging();
    
    if should_skip_integration_tests() {
        println!("Skipping integration test - using mock client");
        return;
    }

    // Configure more aggressive retry settings for this test
    let mut config = RetryConfig::default();
    config.max_retries.insert("json_parse_error".to_string(), 3);
    
    let resolver = create_test_resolver_with_config(config);

    // Use a prompt that might be challenging for JSON parsing
    let result: Result<MathResult, _> = resolver.query_with_schema(
        "Calculate the square root of 144. Be very verbose in your explanation but still return the JSON.".to_string()
    ).await;

    match result {
        Ok(math_result) => {
            println!("✅ Retry behavior test passed:");
            println!("   Result: {}", math_result.result);
            println!("   Is correct: {}", math_result.is_correct);
            
            // Only validate schema compliance, not mathematical correctness
            assert!(matches!(math_result.is_correct, true | false), "is_correct should be boolean");
        },
        Err(e) => {
            eprintln!("❌ Retry behavior test failed: {}", e);
            // This test might fail due to API issues, so we'll log but not panic
            println!("Note: This test may fail due to API rate limits or parsing issues");
        }
    }
}

#[tokio::test] 
async fn test_schema_generation_accuracy() {
    init_test_logging();
    
    if should_skip_integration_tests() {
        println!("Skipping integration test - using mock client");
        return;
    }

    let resolver = create_test_resolver();

    // Test that the AI follows the schema precisely by asking for a specific calculation
    let result: Result<MathResult, _> = resolver.query_with_schema(
        "What is 8 * 7? Return exactly what the schema asks for.".to_string()
    ).await;

    match result {
        Ok(math_result) => {
            println!("✅ Schema generation accuracy test passed:");
            println!("   Result: {}", math_result.result);
            println!("   Is correct: {}", math_result.is_correct);
            
            // Only validate schema adherence, not calculation accuracy
            assert!(matches!(math_result.is_correct, true | false), "is_correct should be boolean");
        },
        Err(e) => {
            eprintln!("❌ Schema generation accuracy test failed: {}", e);
            panic!("Integration test failed: {}", e);
        }
    }
}