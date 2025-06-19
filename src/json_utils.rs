use crate::error::AIError;
use serde::{Serialize, Deserialize};
use tracing::debug;
use regex::Regex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientResponse {
    /// The raw response from the AI model
    pub raw: String,
    /// Text segments that couldn't be parsed as JSON
    pub segmented: Vec<String>,
    /// The extracted JSON response, if any
    pub json_response: Option<String>,
    /// Time taken to process the response in milliseconds
    pub processing_time_ms: u64,
    /// Whether JSON extraction was successful
    pub json_extraction_successful: bool,
    /// Method used to extract JSON (markdown, advanced, line_by_line, raw)
    pub extraction_method: String,
}

impl ClientResponse {
    pub fn new(raw: String, processing_time_ms: u64) -> Self {
        Self {
            raw,
            segmented: Vec::new(),
            json_response: None,
            processing_time_ms,
            json_extraction_successful: false,
            extraction_method: "none".to_string(),
        }
    }
}

/// Process a raw response and extract JSON with metadata
pub fn process_response(raw_response: String, processing_time_ms: u64) -> ClientResponse {
    debug!(response_len = raw_response.len(), "Processing response to extract JSON");
    
    let start_time = std::time::Instant::now();
    let mut response = ClientResponse::new(raw_response.clone(), processing_time_ms);
    
    // First try to extract JSON from markdown code blocks
    if let Some(json_content) = extract_json_from_markdown(&raw_response) {
        debug!(extracted_len = json_content.len(), "Successfully extracted JSON from markdown");
        response.json_response = Some(json_content);
        response.json_extraction_successful = true;
        response.extraction_method = "markdown".to_string();
        response.segmented = segment_non_json_content(&raw_response, response.json_response.as_ref().unwrap());
        return response;
    }
    
    // Try advanced JSON extraction
    if let Some(json_content) = extract_json_advanced(&raw_response) {
        debug!(extracted_len = json_content.len(), "Successfully extracted JSON using advanced method");
        response.json_response = Some(json_content);
        response.json_extraction_successful = true;
        response.extraction_method = "advanced".to_string();
        response.segmented = segment_non_json_content(&raw_response, response.json_response.as_ref().unwrap());
        return response;
    }
    
    // If no JSON found, segment the entire response
    debug!("No JSON found, treating entire response as segmented content");
    response.segmented = vec![raw_response.clone()];
    response.json_response = Some(raw_response.clone()); // Fallback for backward compatibility
    response.extraction_method = "raw".to_string();
    
    let processing_end = std::time::Instant::now();
    response.processing_time_ms += processing_end.duration_since(start_time).as_millis() as u64;
    
    response
}

/// Split the raw response into segments, excluding the JSON content
pub fn segment_non_json_content(raw_response: &str, json_content: &str) -> Vec<String> {
    if let Some(json_start) = raw_response.find(json_content) {
        let mut segments = Vec::new();
        
        // Add content before JSON
        let before = &raw_response[..json_start].trim();
        if !before.is_empty() {
            segments.push(before.to_string());
        }
        
        // Add content after JSON
        let after_start = json_start + json_content.len();
        if after_start < raw_response.len() {
            let after = &raw_response[after_start..].trim();
            if !after.is_empty() {
                segments.push(after.to_string());
            }
        }
        
        segments
    } else {
        // If JSON not found in raw response, return the whole response as segment
        vec![raw_response.to_string()]
    }
}

/// Backward compatibility method - just return the JSON part
pub fn find_json(response: &str) -> String {
    let client_response = process_response(response.to_string(), 0);
    client_response.json_response.unwrap_or_else(|| response.to_string())
}

/// Advanced JSON extraction that searches for valid JSON objects in the response
pub fn extract_json_advanced(response: &str) -> Option<String> {
    debug!("Starting advanced JSON extraction");
    
    // Find all positions where '{' appears
    let open_positions: Vec<usize> = response.char_indices()
        .filter_map(|(i, c)| if c == '{' { Some(i) } else { None })
        .collect();
    
    if open_positions.is_empty() {
        debug!("No opening braces found in response");
        return None;
    }
    
    // Try each opening brace position
    for &start_pos in &open_positions {
        debug!(start_pos = start_pos, "Trying JSON extraction from position");
        
        if let Some(json_str) = find_matching_json_object(&response[start_pos..]) {
            let full_json = &response[start_pos..start_pos + json_str.len()];
            
            // Test if this is valid JSON by attempting to parse it
            if serde_json::from_str::<serde_json::Value>(full_json).is_ok() {
                debug!(json_len = full_json.len(), "Found valid JSON object");
                return Some(full_json.to_string());
            }
        }
    }
    
    // Try line-by-line approach as fallback
    debug!("Trying line-by-line JSON extraction");
    try_line_by_line_json(response)
}

/// Find a complete JSON object starting from the given text
pub fn find_matching_json_object(text: &str) -> Option<String> {
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let chars = text.char_indices();
    
    // Skip to first '{'
    if !text.starts_with('{') {
        return None;
    }
    
    for (i, c) in chars {
        if escape_next {
            escape_next = false;
            continue;
        }
        
        match c {
            '\\' if in_string => escape_next = true,
            '"' => in_string = !in_string,
            '{' if !in_string => brace_count += 1,
            '}' if !in_string => {
                brace_count -= 1;
                if brace_count == 0 {
                    // Found complete JSON object
                    return Some(&text[..=i]).map(|s| s.to_string());
                }
            }
            _ => {}
        }
    }
    
    None
}

/// Try to find JSON by testing each line that starts with '{'
pub fn try_line_by_line_json(response: &str) -> Option<String> {
    let lines: Vec<&str> = response.lines().collect();
    
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') {
            debug!(line_num = i + 1, "Testing line starting with opening brace");
            
            // Try this single line first
            if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
                debug!(line_num = i + 1, "Found valid single-line JSON");
                return Some(trimmed.to_string());
            }
            
            // Try combining this line with subsequent lines
            for end_line in i + 1..lines.len() {
                let combined = lines[i..=end_line].join("\n");
                if serde_json::from_str::<serde_json::Value>(&combined).is_ok() {
                    debug!(start_line = i + 1, end_line = end_line + 1, "Found valid multi-line JSON");
                    return Some(combined);
                }
                
                // Stop if we've gone too far (e.g., more than 50 lines)
                if end_line - i > 50 {
                    break;
                }
            }
        }
    }
    
    None
}

/// Extract JSON from markdown code blocks
pub fn extract_json_from_markdown(response: &str) -> Option<String> {
    // Try different markdown patterns
    let patterns = [
        r"```json\s*\n([\s\S]*?)\n\s*```",
        r"```json([\s\S]*?)```",
        r"```\s*\n([\s\S]*?)\n\s*```",
        r"```([\s\S]*?)```",
    ];
    
    for pattern in &patterns {
        if let Ok(re) = Regex::new(pattern) {
            if let Some(captures) = re.captures(response) {
                if let Some(json_match) = captures.get(1) {
                    let content = json_match.as_str().trim();
                    debug!(pattern = pattern, content_len = content.len(), "Found JSON in markdown");
                    return Some(content.to_string());
                }
            }
        }
    }
    
    None
}

/// Simple JSON extraction from a prompt response  
pub async fn ask_json<F, Fut>(ask_raw_fn: F, prompt: String) -> Result<String, AIError>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<String, AIError>>,
{
    debug!(prompt_len = prompt.len(), "Starting ask_json");
    let raw_response = ask_raw_fn(prompt).await?;
    let json_content = find_json(&raw_response);
    debug!(raw_len = raw_response.len(), json_len = json_content.len(), "Extracted JSON from response");
    Ok(json_content)
}