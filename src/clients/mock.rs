use async_trait::async_trait;
use std::sync::{Arc, Mutex, Weak};
use std::collections::VecDeque;
use crate::{core::LowLevelClient, error::AIError};

/// Mock responses that can be configured
#[derive(Debug, Clone)]
pub enum MockResponse {
    Success(String),
    Error(AIError),
}

/// Shared state for mock responses
#[derive(Debug, Default)]
pub struct MockState {
    responses: VecDeque<MockResponse>,
    fail_on_empty: bool,
}

impl MockState {
    pub fn new(fail_on_empty: bool) -> Self {
        Self {
            responses: VecDeque::new(),
            fail_on_empty,
        }
    }

    pub fn push_response(&mut self, response: MockResponse) {
        self.responses.push_back(response);
    }

    pub fn push_responses(&mut self, responses: Vec<MockResponse>) {
        for response in responses {
            self.responses.push_back(response);
        }
    }

    pub fn next_response(&mut self) -> Result<MockResponse, AIError> {
        self.responses.pop_front().ok_or_else(|| {
            if self.fail_on_empty {
                AIError::Mock("No mock responses available - did you forget to configure the mock?".to_string())
            } else {
                // This shouldn't happen in fail_on_empty mode, but just in case
                AIError::Mock("Mock queue exhausted".to_string())
            }
        })
    }

    pub fn clear(&mut self) {
        self.responses.clear();
    }

    pub fn remaining_count(&self) -> usize {
        self.responses.len()
    }

    pub fn is_empty(&self) -> bool {
        self.responses.is_empty()
    }
}

/// Handle for configuring mock responses from outside
#[derive(Debug)]
pub struct MockHandle {
    state: Arc<Mutex<MockState>>,
}

impl MockHandle {
    fn new(fail_on_empty: bool) -> Self {
        Self {
            state: Arc::new(Mutex::new(MockState::new(fail_on_empty))),
        }
    }

    /// Add a single response to the queue
    pub fn add_response(&self, response: MockResponse) {
        let mut state = self.state.lock().unwrap();
        state.push_response(response);
    }

    /// Add multiple responses to the queue
    pub fn add_responses(&self, responses: Vec<MockResponse>) {
        let mut state = self.state.lock().unwrap();
        state.push_responses(responses);
    }

    /// Add a successful JSON response
    pub fn add_json_response(&self, json: &str) {
        self.add_response(MockResponse::Success(json.to_string()));
    }

    /// Add multiple successful JSON responses
    pub fn add_json_responses(&self, jsons: Vec<&str>) {
        let responses: Vec<MockResponse> = jsons
            .into_iter()
            .map(|json| MockResponse::Success(json.to_string()))
            .collect();
        self.add_responses(responses);
    }

    /// Add an error response
    pub fn add_error(&self, error: AIError) {
        self.add_response(MockResponse::Error(error));
    }

    /// Clear all queued responses
    pub fn clear(&self) {
        let mut state = self.state.lock().unwrap();
        state.clear();
    }

    /// Get the number of remaining responses
    pub fn remaining_count(&self) -> usize {
        let state = self.state.lock().unwrap();
        state.remaining_count()
    }

    /// Check if the response queue is empty
    pub fn is_empty(&self) -> bool {
        let state = self.state.lock().unwrap();
        state.is_empty()
    }

    /// Get next response (for internal use by MockClient)
    fn next_response(&self) -> Result<MockResponse, AIError> {
        let mut state = self.state.lock().unwrap();
        state.next_response()
    }
}

/// Mock client that fails when no responses are available
#[derive(Debug)]
pub struct MockClient {
    handle: Weak<MockHandle>,
}

impl MockClient {
    /// Create a new MockClient with a weak reference to the handle
    /// Returns (client, strong_handle) where the client holds a weak reference
    pub fn new() -> (Self, Arc<MockHandle>) {
        let handle = Arc::new(MockHandle::new(true)); // fail_on_empty = true
        let weak_handle = Arc::downgrade(&handle);
        
        let client = Self {
            handle: weak_handle,
        };
        
        (client, handle)
    }

    /// Create a MockClient with predefined responses
    pub fn with_responses(responses: Vec<MockResponse>) -> (Self, Arc<MockHandle>) {
        let (client, handle) = Self::new();
        handle.add_responses(responses);
        (client, handle)
    }

    /// Try to get the next response, failing if handle is dropped or no responses available
    fn try_next_response(&self) -> Result<MockResponse, AIError> {
        match self.handle.upgrade() {
            Some(handle) => handle.next_response(),
            None => Err(AIError::Mock(
                "MockHandle has been dropped - mock is no longer controllable".to_string()
            )),
        }
    }
}

impl Clone for MockClient {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}

#[async_trait]
impl LowLevelClient for MockClient {
    async fn ask_raw(&self, _prompt: String) -> Result<String, AIError> {
        match self.try_next_response()? {
            MockResponse::Success(response) => Ok(response),
            MockResponse::Error(error) => Err(error),
        }
    }

    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}

/// Mock client for testing that returns empty responses (legacy)
#[derive(Debug, Clone, Default)]
pub struct MockVoid;

#[async_trait]
impl LowLevelClient for MockVoid {
    async fn ask_raw(&self, _prompt: String) -> Result<String, AIError> {
        Ok("{}".to_string())
    }
    
    fn clone_box(&self) -> Box<dyn LowLevelClient> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_with_responses() {
        let (client, mock_handle) = MockClient::new();
        
        // Configure responses
        mock_handle.add_json_responses(vec![
            r#"{"result": "first"}"#,
            r#"{"result": "second"}"#,
        ]);
        
        // Use the client
        let response1 = client.ask_raw("test1".to_string()).await.unwrap();
        let response2 = client.ask_raw("test2".to_string()).await.unwrap();
        
        assert_eq!(response1, r#"{"result": "first"}"#);
        assert_eq!(response2, r#"{"result": "second"}"#);
        
        // Third call should fail since no more responses
        let response3 = client.ask_raw("test3".to_string()).await;
        assert!(response3.is_err());
        assert!(response3.unwrap_err().to_string().contains("No mock responses available"));
    }

    #[tokio::test]
    async fn test_mock_handle_dropped() {
        let client = {
            let (client, _mock_handle) = MockClient::new();
            // mock_handle is dropped here
            client
        };
        
        // Should fail because handle is dropped
        let result = client.ask_raw("test".to_string()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MockHandle has been dropped"));
    }

    #[tokio::test]
    async fn test_mock_with_errors() {
        let (client, mock_handle) = MockClient::new();
        
        mock_handle.add_response(MockResponse::Success(r#"{"ok": true}"#.to_string()));
        mock_handle.add_error(AIError::Mock("Simulated error".to_string()));
        
        // First call succeeds
        let response1 = client.ask_raw("test1".to_string()).await.unwrap();
        assert_eq!(response1, r#"{"ok": true}"#);
        
        // Second call fails with our error
        let response2 = client.ask_raw("test2".to_string()).await;
        assert!(response2.is_err());
        assert!(response2.unwrap_err().to_string().contains("Simulated error"));
    }

    #[tokio::test]
    async fn test_runtime_mock_configuration() {
        let (client, mock_handle) = MockClient::new();
        
        // Initially no responses - should fail
        let result = client.ask_raw("test".to_string()).await;
        assert!(result.is_err());
        
        // Add a response at runtime
        mock_handle.add_json_response(r#"{"added": "later"}"#);
        
        // Now it should work
        let response = client.ask_raw("test".to_string()).await.unwrap();
        assert_eq!(response, r#"{"added": "later"}"#);
        
        // Check handle state
        assert!(mock_handle.is_empty());
        assert_eq!(mock_handle.remaining_count(), 0);
    }
}