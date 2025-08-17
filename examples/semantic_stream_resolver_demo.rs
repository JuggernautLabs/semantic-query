//! Streaming SemanticItem<T> demo using QueryResolver::query_semantic_stream.
//!
//! This example simulates a model stream using `tokio::io::duplex` and feeds it into
//! the resolver-level streaming API. It demonstrates how to interleave Text and Data(T)
//! items and where to hook realtime toolcalls.

use std::default;

use futures_util::{pin_mut, StreamExt}; // for pin_mut + .next()
use schemars::JsonSchema;
use serde::Deserialize;
use semantic_query::core::{QueryResolver, RetryConfig};
use semantic_query::semantic::SemanticItem;

#[derive(Debug, Deserialize, JsonSchema)]
struct Finding {
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env (RUST_LOG etc.) and init tracing
    let _ = dotenvy::dotenv();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // Simulate a streaming model output
    let (mut tx, rx) = tokio::io::duplex(2048);
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let _ = tx.write_all(b"Hello user, here is an update: ").await;
        let _ = tx.write_all(br#"{"message":"service warming up"}"#).await;
        let _ = tx.write_all(b" and more text before the final object ").await;
        let _ = tx.write_all(br#"{"message":"all systems go"}"#).await;
    });

    // The resolver isn't actually used by the streaming API (it’s a thin wrapper),
    // but we construct one to mirror real usage.
    let resolver = QueryResolver::new(semantic_query::clients::mock::MockVoid, RetryConfig::default());

    // Stream SemanticItem<Finding> as data is discovered
    let stream = resolver.query_semantic_stream::<Finding, _>(rx, 1024);
    pin_mut!(stream);
    println!("=== Streaming SemanticItem<Finding> ===");
    while let Some(item) = stream.next().await {
        match item {
            SemanticItem::Text(t) => println!("text: {}", t.text),
            SemanticItem::Data(d) => {
                println!("data: {}", d.message);
                // Hook: trigger a toolcall here based on `d`
            }
            default => continue
        }
    }

    Ok(())
}
