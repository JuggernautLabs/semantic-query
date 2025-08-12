use semantic_query::{core::{QueryResolver, RetryConfig}, clients::mock::MockVoid};
use serde::{Deserialize, Serialize};
use schemars::{JsonSchema, schema_for};

/// A comprehensive analysis result with rich semantic documentation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "Analysis Result", description = "Comprehensive analysis of code with confidence scoring and evidence")]
pub struct EnhancedAnalysisResult {
    /// Confidence score indicating how certain the analysis is
    /// Range: 0.0 (no confidence) to 1.0 (complete confidence)
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    
    /// The primary finding or conclusion from the analysis
    /// Should be a clear, concise statement of what was discovered
    pub finding: String,
    
    /// List of supporting evidence that backs up the finding
    /// Each item should be a specific observation or fact
    #[schemars(description = "Supporting evidence for the finding")]
    pub evidence: Vec<String>,
    
    /// Severity classification of the finding
    #[schemars(description = "Impact level of the finding")]
    pub severity: SeverityLevel,
    
    /// Optional metadata for additional context
    /// Use for tool-specific information, line numbers, etc.
    pub metadata: Option<std::collections::HashMap<String, String>>,
    
    /// Categories this finding belongs to
    /// Helps with organization and filtering
    #[schemars(description = "Classification categories for this finding")]
    pub categories: Vec<Category>,
}

/// Severity levels with clear semantic meaning
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Severity classification system")]
pub enum SeverityLevel {
    /// Informational finding, no action required
    #[schemars(description = "Informational only, no immediate action needed")]
    Info,
    /// Low impact issue, should be addressed when convenient
    #[schemars(description = "Low priority issue, address when convenient")]
    Low,
    /// Medium impact issue, should be addressed in next sprint
    #[schemars(description = "Medium priority, address in next development cycle")]
    Medium,
    /// High impact issue, should be addressed immediately
    #[schemars(description = "High priority, requires immediate attention")]
    High,
    /// Critical security or stability issue, fix immediately
    #[schemars(description = "Critical issue requiring immediate fix")]
    Critical,
}

/// Analysis categories for classification
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Categories for classifying analysis findings")]
pub enum Category {
    /// Security-related findings
    #[schemars(description = "Security vulnerabilities or concerns")]
    Security,
    /// Performance bottlenecks or inefficiencies
    #[schemars(description = "Performance issues or optimization opportunities")]
    Performance,
    /// Code quality and maintainability issues
    #[schemars(description = "Code quality, style, or maintainability concerns")]
    Quality,
    /// Functional bugs or logical errors
    #[schemars(description = "Functional bugs or incorrect behavior")]
    Bug,
    /// Documentation or comment issues
    #[schemars(description = "Missing or incorrect documentation")]
    Documentation,
}

/// Code review result with comprehensive feedback
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "Code Review Result", description = "Comprehensive code review with scoring and detailed feedback")]
pub struct DetailedCodeReview {
    /// Overall quality score from 1 to 10
    /// 1-3: Poor quality, major issues
    /// 4-6: Fair quality, some issues  
    /// 7-8: Good quality, minor issues
    /// 9-10: Excellent quality
    #[schemars(range(min = 1, max = 10))]
    pub quality_score: u8,
    
    /// Specific issues found during review
    #[schemars(description = "List of all issues identified in the code")]
    pub issues: Vec<DetailedIssue>,
    
    /// Positive aspects and strengths of the code
    #[schemars(description = "Good practices and positive aspects found")]
    pub strengths: Vec<String>,
    
    /// Specific actionable recommendations
    #[schemars(description = "Concrete steps to improve the code")]
    pub recommendations: Vec<Recommendation>,
    
    /// Estimated time to address all issues
    #[schemars(description = "Estimated development time to fix issues (in hours)")]
    pub estimated_fix_time_hours: Option<f32>,
}

/// Detailed issue with rich metadata
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "A specific issue found during code analysis")]
pub struct DetailedIssue {
    /// Type of issue found
    pub issue_type: IssueType,
    
    /// Severity level of this specific issue
    pub severity: SeverityLevel,
    
    /// Clear description of what the issue is
    pub description: String,
    
    /// Specific location where the issue occurs
    pub location: Option<IssueLocation>,
    
    /// Suggested fix with implementation details
    pub suggested_fix: Option<String>,
    
    /// Reference to documentation or best practices
    #[schemars(description = "URL or reference to relevant documentation")]
    pub reference: Option<String>,
}

/// Types of issues that can be found
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Classification of different types of code issues")]
pub enum IssueType {
    /// Security vulnerability
    #[schemars(description = "Security vulnerability or weakness")]
    Security,
    /// Performance bottleneck
    #[schemars(description = "Performance issue or inefficiency")]
    Performance,
    /// Code style violation
    #[schemars(description = "Code style or formatting issue")]
    Style,
    /// Functional bug
    #[schemars(description = "Logic error or functional bug")]
    Bug,
    /// Maintainability concern
    #[schemars(description = "Code that is hard to maintain or understand")]
    Maintainability,
    /// Missing or poor testing
    #[schemars(description = "Inadequate test coverage or poor test quality")]
    Testing,
}

/// Location information for an issue
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Specific location where an issue was found")]
pub struct IssueLocation {
    /// File path where the issue occurs
    pub file: Option<String>,
    
    /// Line number (1-indexed)
    #[schemars(range(min = 1))]
    pub line: Option<u32>,
    
    /// Column number (1-indexed)
    #[schemars(range(min = 1))]
    pub column: Option<u32>,
    
    /// Function or method name where issue occurs
    pub function: Option<String>,
}

/// Actionable recommendation for improvement
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(description = "Specific recommendation for code improvement")]
pub struct Recommendation {
    /// Brief title of the recommendation
    pub title: String,
    
    /// Detailed description of what to do
    pub description: String,
    
    /// Priority level for this recommendation
    pub priority: SeverityLevel,
    
    /// Estimated implementation effort
    #[schemars(description = "Estimated hours to implement this recommendation")]
    pub effort_hours: Option<f32>,
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    println!("🚀 Rich Schema Documentation Demo");
    println!("This shows how to use doc comments and schemars attributes for rich AI prompts\n");

    let client = MockVoid;
    let _resolver = QueryResolver::new(client.clone(), RetryConfig::default());
    
    // Generate and display the enhanced schema
    println!("=== Enhanced Analysis Result Schema ===");
    let schema = schema_for!(EnhancedAnalysisResult);
    let schema_json = serde_json::to_string_pretty(&schema)?;
    println!("{}\n", schema_json);
    
    println!("=== Detailed Code Review Schema ===");
    let review_schema = schema_for!(DetailedCodeReview);
    let review_schema_json = serde_json::to_string_pretty(&review_schema)?;
    println!("{}\n", review_schema_json);
    
    // Demonstrate how the prompt becomes structure-agnostic
    println!("=== Structure-Agnostic Prompt Example ===");
    let simple_prompt = "Analyze this code for security issues".to_string();
    
    let augmented_prompt = _resolver.augment_prompt_with_schema::<EnhancedAnalysisResult>(simple_prompt.clone());
    println!("Original prompt: '{}'", simple_prompt);
    println!("Length after schema augmentation: {} characters", augmented_prompt.len());
    println!("Schema provides all structural details automatically!\n");
    
    // Show the benefits
    println!("=== Key Benefits ===");
    println!("✅ Rich semantic documentation embedded in struct definitions");
    println!("✅ Automatic validation rules (ranges, examples, descriptions)");
    println!("✅ Enum variants with clear semantic meaning");
    println!("✅ Structure-agnostic prompts (just say what you want)");
    println!("✅ Self-documenting code with schema generation");
    println!("✅ AI gets comprehensive field meanings automatically");
    
    Ok(())
}
