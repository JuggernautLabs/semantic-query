# Client Implementations

A Rust library providing schema-aware AI client implementations with automatic JSON schema generation and prompt augmentation.

## Quick Start: 30 Seconds to AI-Powered Responses

```rust
use client_implementations::client::{QueryResolver, RetryConfig};
use client_implementations::claude::ClaudeClient;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Analysis result with automatic schema generation
#[derive(Debug, Deserialize, JsonSchema)]
struct CodeAnalysis {
    /// Confidence score from 0.0 to 1.0
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    /// Primary finding from the analysis
    pub finding: String,
    /// Supporting evidence
    pub evidence: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Client reads ANTHROPIC_API_KEY from environment/.env automatically
    let client = ClaudeClient::new()?;
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    // Simple semantic prompt - schema auto-generated and appended!
    let result: CodeAnalysis = resolver.query_with_schema(
        "Analyze this Rust function for potential issues: fn unsafe_function() { /* code */ }".to_string()
    ).await?;
    
    println!("Analysis: {} (confidence: {:.2})", result.finding, result.confidence);
    Ok(())
}
```

**What happens**: Your simple prompt gets automatically expanded with a complete JSON schema. The AI actually receives:

<details>
<summary>Click to see the full prompt sent to the AI</summary>

```
Analyze this Rust function for potential issues: fn unsafe_function() { /* code */ }

Please respond with JSON that matches this exact schema:

{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CodeAnalysis",
  "type": "object",
  "required": ["confidence", "finding", "evidence"],
  "properties": {
    "confidence": {
      "description": "Confidence score from 0.0 to 1.0",
      "type": "number",
      "format": "double",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "finding": {
      "description": "Primary finding from the analysis",
      "type": "string"
    },
    "evidence": {
      "description": "Supporting evidence",
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  }
}

Your response must be valid JSON that can be parsed into this structure. Include all required fields and follow the specified types.
```
</details>

**Environment Setup**: Create `.env` file with `ANTHROPIC_API_KEY=your_key_here` or set environment variable.

## Overview

This library transforms how you interact with AI APIs by automatically generating rich JSON schemas from your Rust structs and appending them to prompts. No more manually maintaining JSON formats or dealing with inconsistent AI responses.

## Key Features

- 🤖 **Schema-Aware AI Clients**: Automatic JSON schema generation and prompt augmentation
- 📝 **Rich Documentation**: Embed semantic meaning directly in struct definitions using doc comments
- 🔒 **Type Safety**: Compile-time validation ensures response structs implement required traits
- 🎯 **Structure-Agnostic Prompts**: Focus on what you want, not how to format the response
- 🔄 **Future-Proof**: Changes to structs automatically update AI prompts
- 🌐 **Environment Integration**: Seamless `.env` and environment variable support

## Detailed Quick Start

### 1. Add to your `Cargo.toml`:

```toml
[dependencies]
client-implementations = { path = "path/to/client-implementations" }
schemars = "0.8"
serde = { version = "1.0", features = ["derive"] }
```

### 2. Define your response structure with rich documentation:

> **💡 Key Feature**: Your normal Rust doc comments (`///`) automatically become AI prompt documentation! No separate schema files needed.

```rust
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// Comprehensive code analysis result.
/// 
/// This structure contains all the information from an AI-powered
/// code analysis, including confidence metrics and detailed findings.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(title = "Code Analysis")]  // Optional: override title
struct AnalysisResult {
    /// Confidence score indicating how certain the analysis is.
    /// 
    /// Range: 0.0 (completely uncertain) to 1.0 (completely confident).
    /// Values below 0.5 suggest the analysis may be unreliable.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    
    /// Primary finding or conclusion from the analysis.
    /// 
    /// This should be a clear, actionable summary of what was discovered.
    /// For security issues, include the vulnerability type and impact.
    pub finding: String,
    
    /// Supporting evidence for the finding.
    /// 
    /// Each item should be a specific observation or code snippet
    /// that backs up the primary finding. Include line numbers when possible.
    pub evidence: Vec<String>,
    
    /// Severity level of any issues found
    pub severity: SeverityLevel,
}

/// Represents different levels of issue severity.
/// 
/// This classification helps prioritize which issues to address first
/// based on their potential impact and urgency.
#[derive(Debug, Deserialize, JsonSchema)]
enum SeverityLevel {
    /// Informational finding - no immediate action required.
    /// 
    /// These are observations that might be useful for understanding
    /// the code but don't represent actual problems.
    Info,
    
    /// Low priority issue - address when convenient.
    /// 
    /// Minor improvements or style issues that should be fixed
    /// but don't impact functionality or security.
    Low,
    
    /// Medium priority issue - address in next development cycle.
    /// 
    /// Issues that could impact maintainability or minor functionality
    /// but don't pose immediate risks.
    Medium,
    
    /// High priority issue - requires immediate attention.
    /// 
    /// Critical bugs, security vulnerabilities, or issues that could
    /// cause system instability or data loss.
    High,
}
```

**🚀 What happens automatically:**
- Struct doc comments → Schema `description` field
- Field doc comments → Property `description` fields  
- Enum doc comments → Type `description` field
- Enum variant doc comments → Individual variant descriptions
- Multi-line comments preserved with paragraph breaks

### 3. Use the schema-aware client:

```rust
use client_implementations::client::{QueryResolver, RetryConfig};
use client_implementations::claude::ClaudeClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Client automatically reads ANTHROPIC_API_KEY from environment/.env
    let client = ClaudeClient::new()?;
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    // Simple, semantic prompt - schema gets auto-generated and appended!
    let result: AnalysisResult = resolver.query_with_schema(
        "Analyze this Rust code for potential issues".to_string()
    ).await?;
    
    println!("Analysis: {} (confidence: {:.2})", result.finding, result.confidence);
    Ok(())
}
```

## What Happens Behind the Scenes

### The Transformation Process

When you call `query_with_schema::<AnalysisResult>(prompt)`, here's what happens:

#### 1. **Schema Generation** (Compile Time)
```rust
// Your struct definition
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(title = "Code Analysis", description = "AI-powered code analysis")]
struct AnalysisResult {
    /// Confidence score from 0.0 to 1.0
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
    // ... other fields
}

// Automatically generates this JSON Schema:
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Code Analysis", 
  "description": "AI-powered code analysis with confidence scoring",
  "type": "object",
  "required": ["confidence", "finding", "evidence", "severity"],
  "properties": {
    "confidence": {
      "description": "Confidence score from 0.0 (uncertain) to 1.0 (very confident)",
      "type": "number",
      "format": "double",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "finding": {
      "description": "Clear, actionable summary of what was discovered",
      "type": "string"
    },
    "evidence": {
      "description": "List of specific observations that support the conclusion",
      "type": "array",
      "items": {"type": "string"}
    },
    "severity": {
      "description": "Severity level of any issues found",
      "oneOf": [
        {"description": "Informational finding", "type": "string", "enum": ["Info"]},
        {"description": "Low priority issue", "type": "string", "enum": ["Low"]},
        {"description": "Medium priority issue", "type": "string", "enum": ["Medium"]},
        {"description": "High priority issue", "type": "string", "enum": ["High"]}
      ]
    }
  }
}
```

#### 2. **Prompt Augmentation** (Runtime)
```rust
// Your simple, semantic prompt:
let original_prompt = "Analyze this Rust code for potential issues";

// Gets automatically transformed into:
let augmented_prompt = r#"Analyze this Rust code for potential issues

Please respond with JSON that matches this exact schema:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Code Analysis",
  "description": "AI-powered code analysis with confidence scoring",
  "type": "object",
  "required": ["confidence", "finding", "evidence", "severity"],
  "properties": {
    "confidence": {
      "description": "Confidence score from 0.0 (uncertain) to 1.0 (very confident)",
      "type": "number",
      "format": "double",
      "minimum": 0.0,
      "maximum": 1.0
    },
    "finding": {
      "description": "Clear, actionable summary of what was discovered",
      "type": "string"
    },
    "evidence": {
      "description": "List of specific observations that support the conclusion",
      "type": "array",
      "items": {"type": "string"}
    },
    "severity": {
      "oneOf": [
        {"description": "Informational finding", "type": "string", "enum": ["Info"]},
        {"description": "Low priority issue", "type": "string", "enum": ["Low"]},
        {"description": "Medium priority issue", "type": "string", "enum": ["Medium"]},
        {"description": "High priority issue", "type": "string", "enum": ["High"]}
      ]
    }
  }
}
```

Your response must be valid JSON that can be parsed into this structure. Include all required fields and follow the specified types and constraints."#;


#### 3. **AI Interaction & Response Processing**
```rust
// Request sent to AI with rich schema
let raw_response = client.ask_raw(augmented_prompt).await?;

// Multi-stage JSON extraction using json_utils module
let json_content = json_utils::find_json(&raw_response);

// 1. Try markdown code block extraction
if let Some(json) = json_utils::extract_json_from_markdown(&raw_response) {
    // Found: ```json { ... } ```
    return Ok(json);
}

// 2. Try advanced JSON object detection
if let Some(json) = json_utils::extract_json_advanced(&raw_response) {
    // Found complete JSON object by brace matching
    return Ok(json);
}

// 3. Line-by-line fallback
// Searches for lines starting with '{' and builds complete objects
```

#### 4. **Type-Safe Deserialization**
```rust
// Parse JSON into your struct with full error context
match serde_json::from_str::<AnalysisResult>(&json_response) {
    Ok(analysis) => {
        // Compile-time guaranteed fields are present
        println!("Confidence: {}", analysis.confidence); // f64, validated 0.0-1.0
        println!("Severity: {:?}", analysis.severity);   // Enum, validated variants
        Ok(analysis)
    },
    Err(e) => {
        // Detailed error with retry logic
        // Shows exactly which field failed and why
        Err(QueryResolverError::JsonDeserialization(e, raw_response))
    }
}
```

### Advanced Schema Features in Action

#### **Nested Structures**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct SecurityReport {
    pub vulnerabilities: Vec<Vulnerability>,
    pub overall_score: u8,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "A specific security vulnerability")]
struct Vulnerability {
    /// Type of vulnerability
    #[schemars(description = "Category like 'SQL Injection', 'XSS', 'Buffer Overflow'")]
    pub vuln_type: String,
    
    /// Source location
    pub location: Option<SourceLocation>,
}
```

**Generates nested schema with definitions:**
```json
{
  "type": "object",
  "properties": {
    "vulnerabilities": {
      "type": "array", 
      "items": {"$ref": "#/definitions/Vulnerability"}
    }
  },
  "definitions": {
    "Vulnerability": {
      "description": "A specific security vulnerability",
      "type": "object",
      "properties": {
        "vuln_type": {
          "description": "Category like 'SQL Injection', 'XSS', 'Buffer Overflow'",
          "type": "string"
        }
      }
    }
  }
}
```

#### **Constraint Validation**
```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct PerformanceMetrics {
    /// Response time in milliseconds
    #[schemars(range(min = 0))]
    pub response_time_ms: u64,
    
    /// CPU usage percentage  
    #[schemars(range(min = 0.0, max = 100.0))]
    pub cpu_usage_percent: f64,
    
    /// Memory usage in bytes
    #[schemars(range(min = 0))]
    pub memory_bytes: u64,
}
```

**AI receives precise constraints:**
```json
{
  "response_time_ms": {
    "type": "integer",
    "format": "uint64", 
    "minimum": 0.0
  },
  "cpu_usage_percent": {
    "type": "number",
    "format": "double",
    "minimum": 0.0,
    "maximum": 100.0
  }
}
```

### Error Handling & Retry Logic

#### **Automatic Retry on Parse Failures**
```rust
// If initial parsing fails:
match serde_json::from_str::<T>(&response) {
    Err(json_err) => {
        // Try advanced JSON extraction
        if let Some(extracted) = extract_json_advanced(&raw_response) {
            // Retry parsing with extracted JSON
            match serde_json::from_str::<T>(&extracted) {
                Ok(parsed) => return Ok(parsed),
                Err(_) => {
                    // Add context to retry prompt
                    let retry_prompt = format!(
                        "{}\n\nPrevious attempt failed: {}. Please fix and respond with valid JSON.",
                        original_prompt, json_err
                    );
                    // Retry with enhanced prompt
                }
            }
        }
    }
}
```

#### **Intelligent Error Context**
```rust
// Detailed error information for debugging
pub enum QueryResolverError {
    JsonDeserialization(serde_json::Error, String), // Error + raw response
    Ai(AIError),                                     // Network/API errors
}

// Usage provides rich context:
match resolver.query_with_schema::<Analysis>(prompt).await {
    Err(QueryResolverError::JsonDeserialization(err, raw)) => {
        eprintln!("JSON Parse Error: {}", err);
        eprintln!("Raw Response: {}", raw);
        eprintln!("Expected schema: {}", schema_for!(Analysis));
    }
}
```


### The Old Way vs The New Way

#### **Before: Manual Schema Maintenance**
```rust
// Every time you change your struct, you must:
// 1. Update the JSON schema in the prompt string
// 2. Hope you didn't make a typo
// 3. Remember to update all related prompts
// 4. Debug mismatched responses manually

let prompt = r#"
Analyze code and return JSON:
{
  "confidence": 0.0-1.0,           // ← Typo: should be number, not string
  "finding": "string",
  "evidence": ["array"],
  "severity": "Info|Low|High"      // ← Forgot to add "Medium" 
}
"#;

// Struct changed but prompt wasn't updated:
struct AnalysisResult {
    pub confidence: f64,
    pub finding: String,
    pub evidence: Vec<String>,
    pub severity: SeverityLevel,
    pub recommendations: Vec<String>, // ← New field not in prompt!
}
```

#### **After: Automatic Schema Sync**
```rust
// Add a field to your struct:
#[derive(Debug, Deserialize, JsonSchema)]
struct AnalysisResult {
    pub confidence: f64,
    pub finding: String, 
    pub evidence: Vec<String>,
    pub severity: SeverityLevel,
    
    /// Actionable recommendations for improvement
    #[schemars(description = "Specific steps to address the findings")]
    pub recommendations: Vec<String>, // ← Automatically included in all prompts!
}

// That's it! All prompts automatically get the updated schema.
// No manual JSON maintenance, no sync issues, no typos.
```

This approach transforms AI interaction from error-prone manual JSON wrangling to type-safe, automatically synchronized communication.

## Environment Configuration

The client automatically handles environment variables:

```bash
# .env file
ANTHROPIC_API_KEY=your_api_key_here
```

Or set environment variables directly:
```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

## Advanced Features

### Custom Retry Configuration

```rust
use std::collections::HashMap;

let mut retry_config = RetryConfig::default();
retry_config.max_retries.insert("rate_limit".to_string(), 3);
retry_config.max_retries.insert("json_parse_error".to_string(), 5);

let resolver = QueryResolver::new(client, retry_config);
```

### Response Processing

```rust
// Get detailed response information
let response = resolver.query_with_schema::<AnalysisResult>(prompt).await?;

// Access raw response details if needed
let client_response = json_utils::process_response(raw_response, processing_time);
println!("Extraction method: {}", client_response.extraction_method);
println!("Processing time: {}ms", client_response.processing_time_ms);
```

### Rich Schema Documentation

```rust
/// Complex analysis with nested structures
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(title = "Security Analysis", description = "Comprehensive security assessment")]
struct SecurityAnalysis {
    /// Overall security score from 1-10
    #[schemars(range(min = 1, max = 10))]
    pub score: u8,
    
    /// Specific vulnerabilities found
    pub vulnerabilities: Vec<Vulnerability>,
    
    /// Recommended security improvements
    #[schemars(description = "Prioritized list of security enhancements")]
    pub recommendations: Vec<Recommendation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(description = "A specific security vulnerability")]
struct Vulnerability {
    /// Type of vulnerability (e.g., "SQL Injection", "XSS")
    pub vulnerability_type: String,
    
    /// Location where vulnerability was found
    pub location: Option<SourceLocation>,
    
    /// Severity level
    pub severity: SeverityLevel,
    
    /// Suggested fix
    #[schemars(description = "Specific steps to remediate this vulnerability")]
    pub fix: String,
}
```

## Supported AI Providers

### Claude (Anthropic)
- ✅ Full implementation with caching support
- ✅ Environment variable configuration
- ✅ Rate limiting and retry logic
- ✅ Advanced JSON extraction

### Planned
- 🔄 OpenAI GPT models
- 🔄 Local model support
- 🔄 Custom endpoint support

## Examples

Run the examples to see the library in action:

```bash
# Basic schema demonstration
cargo run --example schema_demo

# Rich documentation features
cargo run --example rich_schema_demo

# Doc comment demonstration
cargo run --example doc_comment_test
```

## Testing

The library includes comprehensive integration tests that verify real API behavior:

```bash
# Run all tests (requires ANTHROPIC_API_KEY)
cargo test

# Run integration tests specifically
cargo test --test integration_tests

# Run with output to see test results
cargo test --test integration_tests -- --nocapture
```

**Note**: Integration tests require a valid `ANTHROPIC_API_KEY` in your environment or `.env` file. Without it, tests will be skipped automatically.

The integration tests verify:
- ✅ Basic schema-aware queries work correctly
- ✅ Complex schemas with enums and constraints
- ✅ Schema constraint validation (ranges, types)
- ✅ Retry behavior for parsing failures
- ✅ Accurate schema generation and AI adherence

## Benefits

### For Developers
- **Productivity**: No more manual JSON schema maintenance
- **Reliability**: Type-safe responses with automatic validation
- **Maintainability**: Documentation lives with code
- **Flexibility**: Easy to add new fields or change structures

### For AI Interactions
- **Consistency**: Rich schemas ensure consistent AI responses
- **Clarity**: Semantic descriptions help AI understand intent
- **Accuracy**: Constraints and examples improve response quality
- **Debugging**: Clear error messages and response processing info

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Your Struct   │    │   Schema Gen     │    │   AI Client     │
│   + Doc Comments│───▶│   + Validation   │───▶│   + Retry Logic │
│   + Attributes  │    │   + Descriptions │    │   + Parsing     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                        │                       │
         │                        ▼                       ▼
         │              ┌──────────────────┐    ┌─────────────────┐
         │              │ Rich JSON Schema │    │  Augmented      │
         └──────────────▶│ with Semantics   │───▶│  Prompt         │
                        └──────────────────┘    └─────────────────┘
```

## Contributing

Contributions welcome! Areas of interest:
- Additional AI provider implementations
- Enhanced schema generation features
- Performance optimizations
- Documentation improvements

## License

[Add your license here]

---

**Transform your AI interactions: from manual JSON wrangling to semantic, type-safe conversations.**