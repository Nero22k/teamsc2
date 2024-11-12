use reqwest::Client;
use serde::Deserialize;
use std::{error::Error, time::{Duration, Instant}};
use crate::Config;

pub struct Auth {
    client: Client,
    config: Config,
}

// This struct is only for deserializing the response
#[derive(Deserialize)]
pub struct AuthResponse {
    access_token: String,
    expires_in: u64,
}

pub struct AuthToken {
    pub access_token: String,
    pub expires_in: u64,
    pub created_at: Instant,
}

impl AuthToken {
    pub fn new(response: AuthResponse) -> Self {
        Self {
            access_token: response.access_token,
            expires_in: response.expires_in,
            created_at: Instant::now(),
        }
    }

    pub fn is_valid(&self) -> bool {
        let elapsed = self.created_at.elapsed();
        let expires_in = Duration::from_secs(self.expires_in);
        elapsed < (expires_in - Duration::from_secs(300)) // 5 minutes before expiration
    }
}

impl Auth {
    pub fn new(config: &Config) -> Self {
        Self {
            client: Client::new(),
            config: config.clone(),
        }
    }

    pub async fn authenticate(&self) -> Result<AuthToken, Box<dyn Error>> {
        let url = format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.config.tenant_id
        );

        let params = [
            ("client_id", self.config.client_id.as_str()),
            ("scope", "https://graph.microsoft.com/.default"),
            ("username", self.config.username.as_str()),
            ("password", self.config.password.as_str()),
            ("grant_type", "password"),
        ];

        let response = self.client
            .post(&url)
            .form(&params)
            .send()
            .await?
            .error_for_status()?
            .json::<AuthResponse>()
            .await?;

        Ok(AuthToken::new(response))
    }
}