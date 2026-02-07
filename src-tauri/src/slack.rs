use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Slack Lists API client
/// Uses Bot Token authentication with slackLists.* methods

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub bot_token: String,
    pub default_list_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackListItem {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub assignee: Option<String>,
    pub due_date: Option<String>,
}

pub struct SlackClient {
    http: Client,
    config_path: PathBuf,
    config: tokio::sync::Mutex<Option<SlackConfig>>,
}

impl SlackClient {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            http: Client::new(),
            config_path: app_data_dir.join("slack_config.json"),
            config: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn load_config(&self) -> Result<(), String> {
        if !self.config_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.config_path)
            .await
            .map_err(|e| format!("Failed to read slack config: {}", e))?;
        let cfg: SlackConfig = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse slack config: {}", e))?;
        *self.config.lock().await = Some(cfg);
        Ok(())
    }

    pub async fn save_config(&self, config: SlackConfig) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create config dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;
        fs::write(&self.config_path, &content)
            .await
            .map_err(|e| format!("Failed to write config: {}", e))?;
        *self.config.lock().await = Some(config);
        Ok(())
    }

    pub async fn is_configured(&self) -> bool {
        self.config.lock().await.is_some()
    }

    pub async fn get_config(&self) -> Option<SlackConfig> {
        self.config.lock().await.clone()
    }

    fn get_token(&self, config: &SlackConfig) -> String {
        config.bot_token.clone()
    }

    /// Fetch items from a Slack List
    pub async fn list_items(&self, list_id: &str) -> Result<Vec<SlackListItem>, String> {
        let config = self.config.lock().await;
        let config = config.as_ref().ok_or("Slack not configured")?;
        let token = self.get_token(config);

        let resp = self
            .http
            .post("https://slack.com/api/slackLists.items.list")
            .bearer_auth(&token)
            .json(&serde_json::json!({ "list_id": list_id }))
            .send()
            .await
            .map_err(|e| format!("Slack API error: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Slack response: {}", e))?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err = body.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
            return Err(format!("Slack API error: {}", err));
        }

        // Parse items from response
        let items = body
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let id = item.get("id")?.as_str()?.to_string();
                        let title = item.get("title").and_then(|v| v.as_str())
                            .unwrap_or("(無題)")
                            .to_string();
                        // Fields contain completion status, assignee, due date
                        let fields = item.get("fields").cloned().unwrap_or_default();
                        let completed = fields
                            .get("completion")
                            .and_then(|v| v.get("checked"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let assignee = fields
                            .get("assignee")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let due_date = fields
                            .get("due_date")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        Some(SlackListItem {
                            id,
                            title,
                            completed,
                            assignee,
                            due_date,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(items)
    }

    /// Create a new item in a Slack List
    pub async fn create_item(
        &self,
        list_id: &str,
        title: &str,
    ) -> Result<SlackListItem, String> {
        let config = self.config.lock().await;
        let config = config.as_ref().ok_or("Slack not configured")?;
        let token = self.get_token(config);

        let resp = self
            .http
            .post("https://slack.com/api/slackLists.items.create")
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "list_id": list_id,
                "item": {
                    "title": title
                }
            }))
            .send()
            .await
            .map_err(|e| format!("Slack API error: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err = body.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
            return Err(format!("Slack API error: {}", err));
        }

        let item_id = body
            .get("item")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(SlackListItem {
            id: item_id,
            title: title.to_string(),
            completed: false,
            assignee: None,
            due_date: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_config_serialization() {
        let config = SlackConfig {
            bot_token: "xoxb-test-token".to_string(),
            default_list_id: Some("L123".to_string()),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SlackConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bot_token, "xoxb-test-token");
        assert_eq!(parsed.default_list_id, Some("L123".to_string()));
    }

    #[test]
    fn test_slack_list_item_serialization() {
        let item = SlackListItem {
            id: "item-1".to_string(),
            title: "タスク".to_string(),
            completed: false,
            assignee: Some("U123".to_string()),
            due_date: Some("2026-03-01".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: SlackListItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "タスク");
        assert_eq!(parsed.assignee, Some("U123".to_string()));
    }

    #[tokio::test]
    async fn test_slack_client_not_configured() {
        let dir = std::env::temp_dir().join(format!("cowork-slack-test-{}", uuid::Uuid::new_v4()));
        let client = SlackClient::new(dir);
        assert!(!client.is_configured().await);
    }

    #[tokio::test]
    async fn test_slack_config_persistence() {
        let dir = std::env::temp_dir().join(format!("cowork-slack-test-{}", uuid::Uuid::new_v4()));
        let client = SlackClient::new(dir.clone());

        let config = SlackConfig {
            bot_token: "xoxb-persist-test".to_string(),
            default_list_id: None,
        };
        client.save_config(config).await.unwrap();
        assert!(client.is_configured().await);

        // New client loading from same path
        let client2 = SlackClient::new(dir.clone());
        client2.load_config().await.unwrap();
        let loaded = client2.get_config().await.unwrap();
        assert_eq!(loaded.bot_token, "xoxb-persist-test");

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
