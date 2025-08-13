use std::env;
use std::io::{self, Write};
use std::time::Duration;
use std::thread;
use std::fs::OpenOptions;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    terminal,
};


/// Trait for types that can retrieve their configuration key from environment variables
pub trait KeyFromEnv {
    /// The environment variable name for this client's API key
    const KEY_NAME: &'static str;
    
    /// Find the API key by checking environment variables first, then .env file
    fn find_key() -> Option<String> {
        // First try to load .env file (silently fail if not found)
        let _ = dotenvy::dotenv();
        
        // Try to get from environment
        env::var(Self::KEY_NAME).ok()
    }
    
    /// Find the API key with user fallback - waits 15 seconds for user input then panics
    fn find_key_with_user() -> String {
        if let Some(key) = Self::find_key() {
            return key;
        }
        
        // Prompt user for input with timeout
        print!("Environment variable {} not found. Please enter the API key (15 second timeout): ", Self::KEY_NAME);
        io::stdout().flush().unwrap();
        
        // Create a channel for communication between threads
        let (sender, receiver) = std::sync::mpsc::channel();
        
        // Spawn thread to read user input
        thread::spawn(move || {
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let _ = sender.send(input.trim().to_string());
            }
        });
        
        // Wait for input with timeout
        let api_key = match receiver.recv_timeout(Duration::from_secs(15)) {
            Ok(input) if !input.is_empty() => input,
            _ => panic!("Timeout waiting for {} input after 15 seconds", Self::KEY_NAME),
        };
        
        // Ask if user wants to save to .env file
        if Self::prompt_save_to_env() {
            if let Err(e) = Self::save_to_env_file(&api_key) {
                eprintln!("Warning: Failed to save to .env file: {}", e);
            } else {
                println!("API key saved to .env file");
            }
        }
        
        api_key
    }
    
    /// Prompt user if they want to save the API key to .env file
    /// Uses single keystroke detection with fallback to Enter
    fn prompt_save_to_env() -> bool {
        print!("Add {} to .env file? (y/N): ", Self::KEY_NAME);
        io::stdout().flush().unwrap();
        
        // Try single keystroke detection first
        if let Ok(response) = Self::read_single_key() {
            println!("{}", response); // Echo the choice
            return response.to_lowercase() == "y";
        }
        
        // Fallback to readline
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_ok() {
            input.trim().to_lowercase() == "y"
        } else {
            false
        }
    }
    
    /// Attempt to read a single keystroke
    fn read_single_key() -> Result<String, Box<dyn std::error::Error>> {
        // Enable raw mode temporarily
        terminal::enable_raw_mode()?;
        
        let result = if event::poll(Duration::from_secs(30))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                match code {
                    KeyCode::Char('y' | 'Y') => Ok("y".to_string()),
                    KeyCode::Char('n' | 'N') => Ok("n".to_string()),
                    KeyCode::Enter => Ok("n".to_string()), // Default to no
                    _ => Ok("n".to_string()), // Any other key defaults to no
                }
            } else {
                Ok("n".to_string())
            }
        } else {
            Ok("n".to_string()) // Timeout defaults to no
        };
        
        terminal::disable_raw_mode()?;
        result
    }
    
    /// Save the API key to .env file
    fn save_to_env_file(api_key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let env_line = format!("{}={}\n", Self::KEY_NAME, api_key);
        
        // Check if .env file exists and if the key is already there
        if let Ok(content) = std::fs::read_to_string(".env") {
            if content.contains(&format!("{}=", Self::KEY_NAME)) {
                // Key already exists, don't duplicate
                return Ok(());
            }
        }
        
        // Append to .env file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(".env")?;
            
        use std::io::Write;
        file.write_all(env_line.as_bytes())?;
        
        Ok(())
    }
}
