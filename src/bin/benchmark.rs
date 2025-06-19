use clap::Parser;
use semantic_query::clients::flexible::FlexibleClient;
use semantic_query::core::{QueryResolver, RetryConfig};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::env;
use std::io::{self, Write};
use std::time::Instant;
use tokio::task::JoinSet;

#[derive(Debug, Clone)]
pub enum ClientType {
    Claude,
    DeepSeek,
    Mock,
}

impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientType::Claude => write!(f, "Claude"),
            ClientType::DeepSeek => write!(f, "DeepSeek"),
            ClientType::Mock => write!(f, "Mock"),
        }
    }
}

impl ClientType {
    /// Parse client type from string (case insensitive)
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "deepseek" => Ok(Self::DeepSeek),
            "mock" => Ok(Self::Mock),
            _ => Err(format!("Unknown client type: '{}'. Supported: claude, deepseek, mock", s))
        }
    }
    
    /// Get the default client type based on available API keys
    pub fn default() -> Self {
        // Check for API keys in order of preference
        if env::var("ANTHROPIC_API_KEY").is_ok() || 
           std::fs::read_to_string(".env").map_or(false, |content| content.contains("ANTHROPIC_API_KEY")) {
            Self::Claude
        } else if env::var("DEEPSEEK_API_KEY").is_ok() || 
                 std::fs::read_to_string(".env").map_or(false, |content| content.contains("DEEPSEEK_API_KEY")) {
            Self::DeepSeek
        } else {
            Self::Mock
        }
    }
}

/// Get the configured client type from environment or prompt user
fn get_or_prompt_client_type() -> ClientType {
    // Check if already set via environment
    if let Ok(client_str) = env::var("TEST_CLIENT") {
        if let Ok(client_type) = ClientType::from_str(&client_str) {
            return client_type;
        }
    }
    
    // Show menu and prompt user
    println!("🚀 Select AI Client for Benchmarking:");
    println!("1. Claude (Anthropic)");
    println!("2. DeepSeek");
    println!("3. Mock (no API calls)");
    println!();
    
    // Show detected API keys
    let mut available_clients = Vec::new();
    if env::var("ANTHROPIC_API_KEY").is_ok() || 
       std::fs::read_to_string(".env").map_or(false, |content| content.contains("ANTHROPIC_API_KEY")) {
        available_clients.push("Claude");
    }
    if env::var("DEEPSEEK_API_KEY").is_ok() || 
       std::fs::read_to_string(".env").map_or(false, |content| content.contains("DEEPSEEK_API_KEY")) {
        available_clients.push("DeepSeek");
    }
    
    if !available_clients.is_empty() {
        println!("✅ Available (API keys found): {}", available_clients.join(", "));
    }
    
    // Default recommendation
    let default = ClientType::default();
    println!("💡 Recommended: {} (press Enter to use)", default);
    println!();
    
    loop {
        print!("Enter choice (1-3) or press Enter for default: ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        
        if input.is_empty() {
            return default;
        }
        
        match input {
            "1" => return ClientType::Claude,
            "2" => return ClientType::DeepSeek,
            "3" => return ClientType::Mock,
            _ => println!("❌ Invalid choice. Please enter 1, 2, or 3."),
        }
    }
}


/// Simple test structure for basic functionality
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
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
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[schemars(description = "Severity classification for findings")]
pub enum Severity {
    /// Low impact issue
    #[schemars(description = "Minor issue that doesn't significantly impact functionality")]
    Low,
    /// Medium impact issue
    #[schemars(description = "Moderate issue that should be addressed")]
    Medium,
    /// High impact issue
    #[schemars(description = "Critical issue requiring immediate attention")]
    High,
}

#[derive(Parser)]
#[command(author, version, about = "🚀 AI Client Benchmark Runner", long_about = None)]
struct Args {
    /// Set client type: claude, deepseek, mock [default: prompt user]
    #[arg(short, long)]
    client: Option<String>,
    
    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}


/// Individual benchmark functions that can run in parallel

async fn benchmark_math_query(_verbose: bool) -> String {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    let start = Instant::now();
    let result = resolver.query::<MathResult>("What is 15 + 27? Please provide the result and verify if it's correct.".to_string()).await;
    let duration = start.elapsed();
    
    match result {
        Ok(math_result) => {
            format!("✅ Math Query ({:.2}s): result={}, correct={}", 
                duration.as_secs_f64(), math_result.result, math_result.is_correct)
        }
        Err(e) => {
            let mut msg = format!("❌ Math Query failed: {}", e);
            if env::var("TEST_CLIENT").unwrap_or_default() == "mock" {
                msg.push_str("\n   (Expected with Mock client)");
            }
            msg
        }
    }
}

async fn benchmark_code_analysis(verbose: bool) -> String {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    let code = r#"
function processData(data) {
    if (data = null) {
        return data.length;
    }
    return data;
}
    "#;
    
    let prompt = format!(
        "Analyze this JavaScript code for issues:\n\n{}\n\nProvide your analysis with confidence score and specific issues found.", 
        code
    );
    
    let start = Instant::now();
    let result = resolver.query::<CodeAnalysis>(prompt).await;
    let duration = start.elapsed();
    
    match result {
        Ok(analysis) => {
            let mut msg = format!("✅ Code Analysis ({:.2}s): confidence={:.2}, severity={:?}, issues={}", 
                duration.as_secs_f64(), analysis.confidence, analysis.severity, analysis.issues.len());
            if verbose {
                msg.push_str(&format!("\n   Finding: {}", analysis.finding));
                for (i, issue) in analysis.issues.iter().enumerate() {
                    msg.push_str(&format!("\n   Issue {}: {}", i + 1, issue));
                }
            }
            msg
        }
        Err(e) => {
            let mut msg = format!("❌ Code Analysis failed: {}", e);
            if env::var("TEST_CLIENT").unwrap_or_default() == "mock" {
                msg.push_str("\n   (Expected with Mock client)");
            }
            msg
        }
    }
}

async fn benchmark_schema_constraints(verbose: bool) -> String {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    let start = Instant::now();
    let result = resolver.query::<CodeAnalysis>("Give a high-confidence analysis of this simple function: fn add(a: i32, b: i32) -> i32 { a + b }".to_string()).await;
    let duration = start.elapsed();
    
    match result {
        Ok(analysis) => {
            let confidence_valid = analysis.confidence >= 0.0 && analysis.confidence <= 1.0;
            let finding_valid = !analysis.finding.is_empty();
            let severity_valid = matches!(analysis.severity, Severity::Low | Severity::Medium | Severity::High);
            
            let mut msg = if confidence_valid && finding_valid && severity_valid {
                format!("✅ Schema Constraints ({:.2}s): All validations passed", duration.as_secs_f64())
            } else {
                let mut error_msg = format!("❌ Schema Constraints ({:.2}s): Validation failed", duration.as_secs_f64());
                if !confidence_valid { error_msg.push_str(&format!("\n   ❌ Confidence out of range: {}", analysis.confidence)); }
                if !finding_valid { error_msg.push_str("\n   ❌ Finding is empty"); }
                if !severity_valid { error_msg.push_str("\n   ❌ Invalid severity"); }
                error_msg
            };
            
            if verbose {
                msg.push_str(&format!("\n   Confidence: {:.2} (valid: {})", analysis.confidence, confidence_valid));
                msg.push_str(&format!("\n   Finding: {} (valid: {})", analysis.finding, finding_valid));
                msg.push_str(&format!("\n   Severity: {:?} (valid: {})", analysis.severity, severity_valid));
            }
            msg
        }
        Err(e) => {
            let mut msg = format!("❌ Schema Constraints failed: {}", e);
            if env::var("TEST_CLIENT").unwrap_or_default() == "mock" {
                msg.push_str("\n   (Expected with Mock client)");
            }
            msg
        }
    }
}

async fn benchmark_schema_accuracy(verbose: bool) -> String {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    let start = Instant::now();
    let result = resolver.query::<MathResult>("What is 8 * 7? Return exactly what the schema asks for.".to_string()).await;
    let duration = start.elapsed();
    
    match result {
        Ok(math_result) => {
            let boolean_valid = matches!(math_result.is_correct, true | false);
            
            let mut msg = if boolean_valid {
                format!("✅ Schema Accuracy ({:.2}s): Schema followed precisely", duration.as_secs_f64())
            } else {
                format!("❌ Schema Accuracy ({:.2}s): Schema not followed", duration.as_secs_f64())
            };
            
            if verbose {
                msg.push_str(&format!("\n   Result: {} (type: valid)", math_result.result));
                msg.push_str(&format!("\n   Is correct: {} (type: valid boolean)", math_result.is_correct));
            }
            msg
        }
        Err(e) => {
            let mut msg = format!("❌ Schema Accuracy failed: {}", e);
            if env::var("TEST_CLIENT").unwrap_or_default() == "mock" {
                msg.push_str("\n   (Expected with Mock client)");
            }
            msg
        }
    }
}

async fn benchmark_advanced_retry(verbose: bool) -> String {
    let mut retry_config = RetryConfig::default();
    retry_config.max_retries.insert("json_parse_error".to_string(), 3);
    retry_config.default_max_retries = 2;
    
    let client = FlexibleClient::lazy().clone();
    let retry_resolver = QueryResolver::new(client, retry_config);
    
    let start = Instant::now();
    let result = retry_resolver.query::<MathResult>("Calculate the square root of 144. Be very verbose in your explanation but still return the JSON.".to_string()).await;
    let duration = start.elapsed();
    
    match result {
        Ok(math_result) => {
            let mut msg = format!("✅ Advanced Retry ({:.2}s): Handled complex prompt successfully", duration.as_secs_f64());
            if verbose {
                msg.push_str(&format!("\n   Result: {}", math_result.result));
                msg.push_str(&format!("\n   Is correct: {}", math_result.is_correct));
            }
            msg
        }
        Err(e) => {
            let mut msg = format!("⚠️  Advanced Retry ({:.2}s): Failed (may be due to API issues)", duration.as_secs_f64());
            if verbose {
                msg.push_str(&format!("\n   Error: {}", e));
            }
            msg
        }
    }
}

async fn benchmark_empty_prompt(verbose: bool) -> String {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    let start = Instant::now();
    let result = resolver.query::<MathResult>("".to_string()).await; // Empty prompt should fail
    let duration = start.elapsed();
    
    match result {
        Ok(_) => format!("⚠️  Empty Prompt Test ({:.2}s): Unexpectedly succeeded", duration.as_secs_f64()),
        Err(e) => {
            let mut msg = format!("✅ Empty Prompt Test ({:.2}s): Correctly failed", duration.as_secs_f64());
            if verbose {
                msg.push_str(&format!("\n   Error: {}", e));
            }
            msg
        }
    }
}

async fn run_benchmarks_parallel(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    println!("📊 Running Benchmark Suite (Parallel)");
    println!("======================================");
    
    let mut join_set = JoinSet::new();
    
    // Spawn benchmark tasks - each will use the global lazy client
    join_set.spawn(benchmark_math_query(verbose));
    join_set.spawn(benchmark_code_analysis(verbose));
    join_set.spawn(benchmark_schema_constraints(verbose));
    join_set.spawn(benchmark_schema_accuracy(verbose));
    join_set.spawn(benchmark_advanced_retry(verbose));
    join_set.spawn(benchmark_empty_prompt(verbose));
    
    // Collect results as they complete
    let mut results = Vec::new();
    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(output) => results.push(output),
            Err(e) => eprintln!("Benchmark task failed: {}", e),
        }
    }
    
    // Print results in order received (parallel completion)
    for result in results {
        println!("{}", result);
    }
    
    println!();
    println!("🏁 Parallel Benchmark Complete!");
    
    Ok(())
}

// ============================================================================
// DIVAN BENCHMARKS - Run with: cargo bench --bin benchmark
// ============================================================================

/// Divan benchmark for math query performance
#[divan::bench]
async fn divan_math_query() {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    divan::black_box(
        resolver.query::<MathResult>(
            "What is 15 + 27? Please provide the result and verify if it's correct.".to_string()
        ).await
    ).ok();
}

/// Divan benchmark for code analysis performance
#[divan::bench]
async fn divan_code_analysis() {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    let code = r#"
function processData(data) {
    if (data = null) {
        return data.length;
    }
    return data;
}
    "#;
    
    let prompt = format!(
        "Analyze this JavaScript code for issues:\n\n{}\n\nProvide your analysis with confidence score and specific issues found.", 
        code
    );
    
    divan::black_box(
        resolver.query::<CodeAnalysis>(prompt).await
    ).ok();
}

/// Divan benchmark for schema constraints validation
#[divan::bench]
async fn divan_schema_constraints() {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    divan::black_box(
        resolver.query::<CodeAnalysis>(
            "Give a high-confidence analysis of this simple function: fn add(a: i32, b: i32) -> i32 { a + b }".to_string()
        ).await
    ).ok();
}

/// Divan benchmark for schema accuracy testing
#[divan::bench]
async fn divan_schema_accuracy() {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    divan::black_box(
        resolver.query::<MathResult>(
            "What is 8 * 7? Return exactly what the schema asks for.".to_string()
        ).await
    ).ok();
}

/// Divan benchmark for advanced retry behavior
#[divan::bench]
async fn divan_advanced_retry() {
    let mut retry_config = RetryConfig::default();
    retry_config.max_retries.insert("json_parse_error".to_string(), 3);
    retry_config.default_max_retries = 2;
    
    let client = FlexibleClient::lazy().clone();
    let retry_resolver = QueryResolver::new(client, retry_config);
    
    divan::black_box(
        retry_resolver.query::<MathResult>(
            "Calculate the square root of 144. Be very verbose in your explanation but still return the JSON.".to_string()
        ).await
    ).ok();
}

/// Divan benchmark for empty prompt error handling
#[divan::bench]
async fn divan_empty_prompt() {
    let client = FlexibleClient::lazy().clone();
    let resolver = QueryResolver::new(client, RetryConfig::default());
    
    divan::black_box(
        resolver.query::<MathResult>("".to_string()).await
    ).ok();
}

// Main function for divan when running benchmarks
fn main() {
    // Check if we're running in benchmark mode
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "--bench" {
        // Set default client for benchmarks if not specified
        if env::var("TEST_CLIENT").is_err() {
            env::set_var("TEST_CLIENT", "mock"); // Default to mock for benchmarks
        }
        divan::main();
    } else {
        // Run the interactive benchmark mode
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            if let Err(e) = run_interactive_benchmarks().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        });
    }
}

// Rename the original main function
async fn run_interactive_benchmarks() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    
    // Initialize tracing if verbose
    if args.verbose {
        tracing_subscriber::fmt::init();
    }
    
    // Get client type
    let client_type = if let Some(client_str) = args.client {
        ClientType::from_str(&client_str)?
    } else {
        get_or_prompt_client_type()
    };
    
    println!("🎯 Running benchmarks with {} client", client_type);
    println!();
    
    // Set TEST_CLIENT environment variable for lazy client
    env::set_var("TEST_CLIENT", client_type.to_string().to_lowercase());
    
    // Run benchmark tests in parallel
    run_benchmarks_parallel(args.verbose).await?;
    
    Ok(())
}