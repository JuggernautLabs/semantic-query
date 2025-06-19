use serde::{Deserialize, Serialize};
use schemars::{JsonSchema, schema_for};

/// This is a comprehensive analysis result structure
/// that demonstrates how doc comments appear in JSON schemas.
/// 
/// It includes multiple fields with different documentation styles.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[schemars(title = "Analysis Result")]
struct AnalysisResult {
    /// The confidence score for this analysis.
    /// This represents how certain the AI is about the findings.
    /// Range: 0.0 (not confident) to 1.0 (very confident)
    pub confidence: f64,
    
    /// Primary finding from the analysis.
    /// 
    /// This should be a clear, actionable summary of what was discovered.
    /// Multiple lines of documentation are supported and will appear
    /// in the schema description.
    pub finding: String,
    
    /// Supporting evidence for the finding
    pub evidence: Vec<String>,
    
    /// The severity level of any issues found
    pub severity: SeverityLevel,
}

/// Represents different levels of issue severity.
/// 
/// This enum helps categorize findings by their impact level,
/// allowing for appropriate prioritization of fixes.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub enum SeverityLevel {
    /// Informational finding - no action required.
    /// These are observations that might be useful but don't require fixes.
    Info,
    
    /// Low priority issue.
    /// Should be addressed when convenient, not urgent.
    Low,
    
    /// Medium priority issue.
    /// Should be addressed in the next development cycle.
    Medium,
    
    /// High priority issue.
    /// Requires immediate attention and should be fixed soon.
    High,
    
    /// Critical issue.
    /// Must be fixed immediately as it poses significant risk.
    Critical,
}

fn main() {
    println!("=== Doc Comments in JSON Schema Demo ===\n");
    
    let schema = schema_for!(AnalysisResult);
    let schema_json = serde_json::to_string_pretty(&schema).unwrap();
    
    println!("Generated Schema:");
    println!("{}", schema_json);
    
    println!("\n=== Key Points ===");
    println!("✅ Struct doc comments become schema description");
    println!("✅ Field doc comments become property descriptions");
    println!("✅ Enum doc comments become variant descriptions");
    println!("✅ Multi-line doc comments are preserved");
    println!("✅ Both /// and /** */ style comments work");
}