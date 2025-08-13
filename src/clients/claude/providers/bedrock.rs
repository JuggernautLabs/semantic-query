use crate::error::{AIError, ClaudeError};
use async_trait::async_trait;
use tracing::{debug, error, info, instrument};

use super::{ClaudeProvider, ClaudeRequest};
use crate::clients::claude::config::ClaudeConfig;
#[cfg(feature = "aws-bedrock-sdk")]
use aws_sdk_bedrockruntime as bedrockrt;
#[cfg(feature = "aws-bedrock-sdk")]
use aws_smithy_types::Blob;

#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug)]
pub struct BedrockProvider {
    config: ClaudeConfig,
    // Note: In a real implementation, you'd include AWS SDK client here
    // For now, we'll just store the config and implement a placeholder
}

impl BedrockProvider {
    #[must_use]
    pub const fn new(config: ClaudeConfig) -> Self {
        Self { config }
    }

    async fn call_bedrock_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        #[cfg(not(feature = "aws-bedrock-sdk"))]
        {
            return Err(AIError::Claude(ClaudeError::Api(
                "AWS Bedrock provider not wired. Enable the optional `aws-bedrock-sdk` feature and provide credentials to call Bedrock Runtime.".to_string()
            )));
        }
        #[cfg(feature = "aws-bedrock-sdk")]
        {
            // Build AWS config and client
            let region = self.config.aws_region.clone().unwrap_or_else(|| "us-east-1".to_string());
            let aws_cfg = aws_config::from_env().region(aws_config::Region::new(region)).load().await;
            let client = bedrockrt::Client::new(&aws_cfg);

            // Build anthropic-style payload for Bedrock messages
            let messages: Vec<serde_json::Value> = request.messages.iter().map(|m| {
                let content_blocks = match &m.content {
                    super::ClaudeMessageContent::Simple(s) => vec![serde_json::json!({"type":"text","text": s})],
                    super::ClaudeMessageContent::Structured(blocks) => blocks.iter().map(|b| serde_json::json!({
                        "type": b.block_type, "text": b.text
                    })).collect(),
                };
                serde_json::json!({"role": m.role, "content": content_blocks})
            }).collect();

            let payload = serde_json::json!({
                "anthropic_version": "bedrock-2023-05-31",
                "max_tokens": request.max_tokens,
                "messages": messages
            });

            let resp = client
                .invoke_model()
                .model_id(&request.model)
                .content_type("application/json")
                .accept("application/json")
                .body(Blob::new(payload.to_string()))
                .send()
                .await
                .map_err(|e| AIError::Claude(ClaudeError::Http(e.to_string())))?;

            let body_bytes = resp.body().as_ref();
            let v: serde_json::Value = serde_json::from_slice(body_bytes)
                .map_err(|e| AIError::Claude(ClaudeError::Http(e.to_string())))?;
            // Extract first content text
            let text = v.get("content").and_then(|c| c.get(0)).and_then(|c0| c0.get("text")).and_then(|t| t.as_str())
                .ok_or_else(|| AIError::Claude(ClaudeError::Api("No content in Bedrock response".into())))?;
            Ok(text.to_string())
        }
    }

}

#[async_trait]
impl ClaudeProvider for BedrockProvider {
    #[instrument(skip(self, request), fields(model = %request.model, region = ?self.config.aws_region))]
    async fn call_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        debug!(
            model = %request.model,
            region = ?self.config.aws_region,
            "Preparing AWS Bedrock API request"
        );

        // Validate AWS configuration
        let region = self.config.aws_region.as_ref().ok_or_else(|| {
            error!("AWS region not configured for Bedrock provider");
            AIError::Claude(ClaudeError::Api("AWS region not configured".to_string()))
        })?;

        info!(
            model = %request.model,
            region = %region,
            "Calling AWS Bedrock API"
        );

        self.call_bedrock_api(request).await
    }

    async fn stream_api(&self, request: &ClaudeRequest) -> Result<std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, AIError>> + Send>>, AIError> {
        #[cfg(not(feature = "aws-bedrock-sdk"))]
        {
            return Err(AIError::Claude(ClaudeError::Api(
                "AWS Bedrock streaming requires the optional `aws-bedrock-sdk` feature".into()
            )));
        }
        #[cfg(feature = "aws-bedrock-sdk")]
        {
            use futures_util::StreamExt;

            // Build AWS client
            let region = self.config.aws_region.clone().unwrap_or_else(|| "us-east-1".to_string());
            let aws_cfg = aws_config::from_env().region(aws_config::Region::new(region)).load().await;
            let client = bedrockrt::Client::new(&aws_cfg);

            // Build payload (with stream: true to hint streaming-capable models)
            let messages: Vec<serde_json::Value> = request.messages.iter().map(|m| {
                let content_blocks = match &m.content {
                    super::ClaudeMessageContent::Simple(s) => vec![serde_json::json!({"type":"text","text": s})],
                    super::ClaudeMessageContent::Structured(blocks) => blocks.iter().map(|b| serde_json::json!({
                        "type": b.block_type, "text": b.text
                    })).collect(),
                };
                serde_json::json!({"role": m.role, "content": content_blocks})
            }).collect();

            let payload = serde_json::json!({
                "anthropic_version": "bedrock-2023-05-31",
                "max_tokens": request.max_tokens,
                "messages": messages,
                "stream": true
            });

            // Try InvokeModelWithResponseStream first; if unsupported by model, fallback to one-shot
            let try_stream = client
                .invoke_model_with_response_stream()
                .model_id(&request.model)
                .content_type("application/json")
                .accept("application/json")
                .body(Blob::new(payload.to_string()))
                .send()
                .await;

            let s = match try_stream {
                Ok(resp) => {
                    // Map the SDK stream into Bytes; different models emit different variants.
                    // We conservatively forward any byte payload parts as-is.
                    let mut inner = resp.body
                        .into_inner();
                    let s = async_stream::try_stream! {
                        while let Some(evt) = inner.next().await {
                            // Each evt is Result<InvokeModelWithResponseStreamOutputBody, _>
                            let evt = evt.map_err(|e| AIError::Claude(ClaudeError::Http(e.to_string())))?;
                            // The event body exposes .into_event() => enum with chunk bytes.
                            // Since exact enum names may change across SDK versions, attempt common accessors.
                            // Prefer .chunk or .payload_part variants with a .bytes() accessor.
                            #[allow(unused_mut)]
                            let mut delivered = false;
                            #[allow(unused_variables)]
                            if let Some(bytes) = evt.bytes() {
                                delivered = true;
                                yield Bytes::copy_from_slice(bytes.as_ref());
                            }
                            // Fallback: try to_string for unknown payloads
                            if !delivered {
                                let s = format!("{}", "");
                                if !s.is_empty() { yield Bytes::from(s); }
                            }
                        }
                    };
                    Box::pin(s.map_err(|e| e))
                }
                Err(_) => {
                    // Fallback to one-shot InvokeModel and yield once
                    let oneshot = client
                        .invoke_model()
                        .model_id(&request.model)
                        .content_type("application/json")
                        .accept("application/json")
                        .body(Blob::new(payload.to_string()))
                        .send()
                        .await
                        .map_err(|e| AIError::Claude(ClaudeError::Http(e.to_string())))?;
                    let body = oneshot.body().as_ref().to_vec();
                    let s = async_stream::try_stream! {
                        yield Bytes::from(body);
                    };
                    Box::pin(s.map_err(|e| e))
                }
            };

            Ok(s)
        }
    }
}

// Example of what a real Bedrock implementation might look like:
/*
use aws_sdk_bedrockruntime as bedrock;
use aws_config::meta::region::RegionProviderChain;

impl BedrockProvider {
    pub async fn new(config: ClaudeConfig) -> Result<Self, AIError> {
        let region_provider = RegionProviderChain::default_provider()
            .or_else(config.aws_region.as_deref().unwrap_or("us-east-1"));
        
        let aws_config = aws_config::from_env()
            .region(region_provider)
            .load()
            .await;
            
        let bedrock_client = bedrock::Client::new(&aws_config);
        
        Ok(Self {
            config,
            client: bedrock_client,
        })
    }

    async fn call_bedrock_api(&self, request: &ClaudeRequest) -> Result<String, AIError> {
        let body = json!({
            "anthropic_version": "bedrock-2023-05-31",
            "max_tokens": request.max_tokens,
            "messages": request.messages
        });

        let response = self.client
            .invoke_model()
            .model_id(&request.model)
            .body(aws_smithy_types::Blob::new(body.to_string()))
            .send()
            .await
            .map_err(|e| AIError::Claude(ClaudeError::Http(e.to_string())))?;

        let response_body = response.body().as_ref();
        let response_json: serde_json::Value = serde_json::from_slice(response_body)
            .map_err(|e| AIError::Claude(ClaudeError::Http(e.to_string())))?;

        let content = response_json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| AIError::Claude(ClaudeError::Api("No content in response".to_string())))?;

        Ok(content.to_string())
    }
}
*/
