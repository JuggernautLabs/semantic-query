use clap::{Parser, ValueEnum};
use std::env;
use std::process::{Command, exit};

#[derive(Clone, Debug, ValueEnum)]
enum ClientType {
    Claude,
    Deepseek,
    Mock,
}

impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientType::Claude => write!(f, "claude"),
            ClientType::Deepseek => write!(f, "deepseek"),
            ClientType::Mock => write!(f, "mock"),
        }
    }
}

#[derive(Parser)]
#[command(author, version, about = "🧪 AI Client Integration Test Runner", long_about = None)]
#[command(after_help = "ENVIRONMENT VARIABLES:
    TEST_CLIENT        Override client type (claude|deepseek|mock)
    ANTHROPIC_API_KEY  API key for Claude client
    DEEPSEEK_API_KEY   API key for DeepSeek client

EXAMPLES:
    test_runner                           # Auto-detect client and run all tests
    test_runner --client claude           # Force Claude client
    test_runner --client mock --verbose   # Use mock client with verbose output
    test_runner --test schema              # Run only tests matching 'schema'
    test_runner --nocapture               # Show test output in real-time")]
struct Args {
    /// Set client type: claude, deepseek, mock [default: auto-detect]
    #[arg(short, long, value_enum)]
    client: Option<ClientType>,

    /// Enable verbose test output
    #[arg(short, long)]
    verbose: bool,

    /// Show test output (equivalent to cargo test -- --nocapture)
    #[arg(long)]
    nocapture: bool,

    /// Run specific test (supports partial matching)
    #[arg(long)]
    test: Option<String>,
}

fn main() {
    let args = Args::parse();
    
    // Set environment variable for client type if specified
    if let Some(client) = &args.client {
        env::set_var("TEST_CLIENT", client.to_string());
    }
    
    // Build cargo test command
    let mut cmd = Command::new("cargo");
    
    cmd.arg("test");
    cmd.arg("--jobs=1");
    if args.verbose {
        cmd.arg("--verbose");
    }
    
    // Add test arguments
    cmd.arg("--");
    
    if args.nocapture {
        cmd.arg("--nocapture");
    }
    
    if let Some(filter) = &args.test {
        cmd.arg(filter);
    }
    
    // Print configuration
    println!("🚀 Starting integration tests...");
    if let Ok(test_client) = env::var("TEST_CLIENT") {
        println!("   Client: {}", test_client);
    } else {
        println!("   Client: auto-detect");
    }
    println!();
    
    // Execute the command
    let status = cmd.status().expect("Failed to execute cargo test");
    
    if status.success() {
        println!("\n✅ All tests completed successfully!");
    } else {
        println!("\n❌ Some tests failed!");
        exit(status.code().unwrap_or(1));
    }
}