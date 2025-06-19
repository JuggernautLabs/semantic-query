# Test Runner

This project now includes a flexible test runner that can work with any AI client through environment variable configuration.

## Key Features

✅ **Dynamic Client Selection**: Tests can run with Claude, DeepSeek, or Mock clients  
✅ **Environment Variable Configuration**: No clap dependency, pure env var driven  
✅ **Lazy Evaluation**: `Box<dyn LowLevelClient>` with lazy initialization  
✅ **Command Line Interface**: Dedicated test runner binary with flags  
✅ **Automatic Fallback**: Falls back to Mock client if API keys aren't available  

## Usage

### Basic Usage

```bash
# Auto-detect client based on API keys and run all tests
cargo run --bin test_runner

# Force specific client
cargo run --bin test_runner -- --client claude
cargo run --bin test_runner -- --client deepseek  
cargo run --bin test_runner -- --client mock

# Run specific tests
cargo run --bin test_runner -- --test schema
cargo run --bin test_runner -- --test retry

# Verbose output
cargo run --bin test_runner -- --verbose --nocapture
```

### Environment Variables

```bash
# Override client type
export TEST_CLIENT=claude
cargo run --bin test_runner

# API Keys (one of these required for real clients)
export ANTHROPIC_API_KEY=your-claude-key
export DEEPSEEK_API_KEY=your-deepseek-key
```

### Direct cargo test (still works)

```bash
# Traditional approach
TEST_CLIENT=mock cargo test

# With API key
ANTHROPIC_API_KEY=your-key cargo test
```

## Architecture

### Components

1. **`test_utils`** module:
   - `get_client_type()`: Determines which client to use
   - `create_test_resolver()`: Creates QueryResolver with dynamic client
   - `should_skip_integration_tests()`: Skip logic for mock client

2. **`test_runner`** binary:
   - Command-line interface with flags
   - Sets environment variables and runs `cargo test`
   - Provides user-friendly output and error handling

3. **Integration Tests**:
   - Use `create_test_resolver()` instead of hardcoded clients
   - Automatically skip when using Mock client
   - Print client configuration info

### Dynamic Client Creation

```rust
// Old approach (hardcoded)
let client = ClaudeClient::new()?;
let resolver = QueryResolver::new(client, RetryConfig::default());

// New approach (dynamic)
let resolver = create_test_resolver(); // Uses env vars
```

## Benefits

🎯 **Flexibility**: Run same tests with different AI providers  
🎯 **CI/CD Friendly**: Easy to configure in different environments  
🎯 **Development**: Quick switching between real and mock clients  
🎯 **No Dependencies**: Removed clap, pure std library  
🎯 **Backward Compatible**: Existing test commands still work  

## Client Priority

1. **Explicit**: `TEST_CLIENT=claude` overrides everything
2. **Claude**: If `ANTHROPIC_API_KEY` is available
3. **DeepSeek**: If `DEEPSEEK_API_KEY` is available  
4. **Mock**: Fallback if no API keys found

This ensures tests can always run, even without API keys configured.