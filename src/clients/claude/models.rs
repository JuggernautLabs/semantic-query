pub struct ClaudeModels;

impl ClaudeModels {
    // Claude 4 Models
    pub const OPUS_4: &'static str = "claude-opus-4-20250514";
    pub const SONNET_4: &'static str = "claude-sonnet-4-20250514";
    
    // Claude 3.7 Models
    pub const SONNET_3_7: &'static str = "claude-3-7-sonnet-20250219";
    pub const SONNET_3_7_LATEST: &'static str = "claude-3-7-sonnet-latest";
    
    // Claude 3.5 Models
    pub const HAIKU_3_5: &'static str = "claude-3-5-haiku-20241022";
    pub const HAIKU_3_5_LATEST: &'static str = "claude-3-5-haiku-latest";
    pub const SONNET_3_5_V2: &'static str = "claude-3-5-sonnet-20241022";
    pub const SONNET_3_5_V2_LATEST: &'static str = "claude-3-5-sonnet-latest";
    pub const SONNET_3_5: &'static str = "claude-3-5-sonnet-20240620";
    
    // Claude 3 Models
    pub const OPUS_3: &'static str = "claude-3-opus-20240229";
    pub const OPUS_3_LATEST: &'static str = "claude-3-opus-latest";
    pub const SONNET_3: &'static str = "claude-3-sonnet-20240229";
    pub const HAIKU_3: &'static str = "claude-3-haiku-20240307";
}

pub struct BedrockModels;

impl BedrockModels {
    // Claude 4 Models
    pub const OPUS_4: &'static str = "anthropic.claude-opus-4-20250514-v1:0";
    pub const SONNET_4: &'static str = "anthropic.claude-sonnet-4-20250514-v1:0";
    
    // Claude 3.7 Models
    pub const SONNET_3_7: &'static str = "anthropic.claude-3-7-sonnet-20250219-v1:0";
    
    // Claude 3.5 Models
    pub const HAIKU_3_5: &'static str = "anthropic.claude-3-5-haiku-20241022-v1:0";
    pub const SONNET_3_5_V2: &'static str = "anthropic.claude-3-5-sonnet-20241022-v2:0";
    pub const SONNET_3_5: &'static str = "anthropic.claude-3-5-sonnet-20240620-v1:0";
    
    // Claude 3 Models
    pub const OPUS_3: &'static str = "anthropic.claude-3-opus-20240229-v1:0";
    pub const SONNET_3: &'static str = "anthropic.claude-3-sonnet-20240229-v1:0";
    pub const HAIKU_3: &'static str = "anthropic.claude-3-haiku-20240307-v1:0";
}

pub struct VertexModels;

impl VertexModels {
    // Claude 4 Models
    pub const OPUS_4: &'static str = "claude-opus-4@20250514";
    pub const SONNET_4: &'static str = "claude-sonnet-4@20250514";
    
    // Claude 3.7 Models
    pub const SONNET_3_7: &'static str = "claude-3-7-sonnet@20250219";
    
    // Claude 3.5 Models
    pub const HAIKU_3_5: &'static str = "claude-3-5-haiku@20241022";
    pub const SONNET_3_5_V2: &'static str = "claude-3-5-sonnet-v2@20241022";
    pub const SONNET_3_5: &'static str = "claude-3-5-sonnet@20240620";
    
    // Claude 3 Models
    pub const OPUS_3: &'static str = "claude-3-opus@20240229";
    pub const SONNET_3: &'static str = "claude-3-sonnet@20240229";
    pub const HAIKU_3: &'static str = "claude-3-haiku@20240307";
}