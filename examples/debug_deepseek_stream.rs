//! Debug DeepSeek streaming implementation

use semantic_query::clients::deepseek::{DeepSeekClient, DeepSeekConfig};
use semantic_query::core::LowLevelClient;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let client = DeepSeekClient::new(DeepSeekConfig::default());
    
    println!("Testing DeepSeek streaming...");
    
    let stream_result = client.stream_raw("Hello world".to_string());
    match stream_result {
        Some(mut stream) => {
            println!("✅ Stream created successfully!");
            let mut count = 0;
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        count += 1;
                        println!("Chunk {}: {} bytes", count, bytes.len());
                        if count >= 5 { break; } // Limit output
                    }
                    Err(e) => {
                        println!("❌ Stream error: {}", e);
                        break;
                    }
                }
            }
        }
        None => {
            println!("❌ Stream creation returned None");
        }
    }

    Ok(())
}