use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use crate::oauth_server;

const AUTH_URL: &str = "https://slack.com/oauth/v2/authorize";
const TOKEN_URL: &str = "https://slack.com/api/oauth.v2.access";

/// OAuth credentials (embedded or user-provided)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackOAuthConfig {
    pub client_id: String,
    pub client_secret: String,
}

/// Persisted auth tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackTokens {
    pub bot_token: String,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
}

/// User-configurable settings (e.g. which list to use)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackSettings {
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
    data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    oauth_config: tokio::sync::Mutex<Option<SlackOAuthConfig>>,
    tokens: tokio::sync::Mutex<Option<SlackTokens>>,
    settings: tokio::sync::Mutex<SlackSettings>,
}

impl SlackClient {
    pub fn new(data_dir: PathBuf, resource_dir: Option<PathBuf>) -> Self {
        Self {
            http: Client::new(),
            data_dir,
            resource_dir,
            oauth_config: tokio::sync::Mutex::new(None),
            tokens: tokio::sync::Mutex::new(None),
            settings: tokio::sync::Mutex::new(SlackSettings {
                default_list_id: None,
            }),
        }
    }

    pub async fn load(&self) -> Result<(), String> {
        // Load OAuth config
        if let Some(config) = self.load_oauth_config().await {
            *self.oauth_config.lock().await = Some(config);
        }

        // Load tokens
        let tokens_path = self.data_dir.join("slack_tokens.json");
        if tokens_path.exists() {
            let content = fs::read_to_string(&tokens_path)
                .await
                .map_err(|e| format!("Slack認証情報を読み込めませんでした: {}", e))?;
            let tokens: SlackTokens = serde_json::from_str(&content)
                .map_err(|e| format!("Slack認証情報の形式が正しくありません: {}", e))?;
            *self.tokens.lock().await = Some(tokens);
        }

        // Load settings
        let settings_path = self.data_dir.join("slack_settings.json");
        if settings_path.exists() {
            let content = fs::read_to_string(&settings_path)
                .await
                .map_err(|e| format!("Slack設定を読み込めませんでした: {}", e))?;
            if let Ok(s) = serde_json::from_str::<SlackSettings>(&content) {
                *self.settings.lock().await = s;
            }
        }

        Ok(())
    }

    async fn load_oauth_config(&self) -> Option<SlackOAuthConfig> {
        // 1. Try bundled resource
        if let Some(ref dir) = self.resource_dir {
            let path = dir.join("slack_oauth.json");
            if let Ok(content) = fs::read_to_string(&path).await {
                if let Ok(config) = serde_json::from_str::<SlackOAuthConfig>(&content) {
                    log::info!(
                        "Slack OAuth config loaded from resources: {}",
                        path.display()
                    );
                    return Some(config);
                }
            }
        }

        // 2. Try user data dir
        let path = self.data_dir.join("slack_oauth_config.json");
        if let Ok(content) = fs::read_to_string(&path).await {
            if let Ok(config) = serde_json::from_str::<SlackOAuthConfig>(&content) {
                log::info!("Slack OAuth config loaded from data dir");
                return Some(config);
            }
        }

        None
    }

    async fn save_tokens(&self, tokens: &SlackTokens) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| format!("フォルダを作成できませんでした: {}", e))?;
        let path = self.data_dir.join("slack_tokens.json");
        let content = serde_json::to_string_pretty(tokens)
            .map_err(|e| format!("データの変換に失敗しました: {}", e))?;
        fs::write(&path, &content)
            .await
            .map_err(|e| format!("ファイルの書き込みに失敗しました: {}", e))?;
        *self.tokens.lock().await = Some(tokens.clone());
        Ok(())
    }

    pub async fn save_settings(&self, settings: SlackSettings) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| format!("フォルダを作成できませんでした: {}", e))?;
        let path = self.data_dir.join("slack_settings.json");
        let content = serde_json::to_string_pretty(&settings)
            .map_err(|e| format!("データの変換に失敗しました: {}", e))?;
        fs::write(&path, &content)
            .await
            .map_err(|e| format!("ファイルの書き込みに失敗しました: {}", e))?;
        *self.settings.lock().await = settings;
        Ok(())
    }

    pub async fn is_configured(&self) -> bool {
        self.oauth_config.lock().await.is_some()
    }

    pub async fn is_authenticated(&self) -> bool {
        self.tokens.lock().await.is_some()
    }

    pub async fn get_team_name(&self) -> Option<String> {
        self.tokens.lock().await.as_ref()?.team_name.clone()
    }

    pub async fn get_settings(&self) -> SlackSettings {
        self.settings.lock().await.clone()
    }

    /// Start the OAuth flow. Returns (auth_url, port, receiver).
    pub async fn start_auth_flow(
        &self,
    ) -> Result<(String, u16, tokio::sync::oneshot::Receiver<Result<String, String>>), String> {
        let config = self.oauth_config.lock().await;
        let config = config
            .as_ref()
            .ok_or("SlackのOAuth設定が見つかりません。開発者に連絡してください。")?;

        let (port, rx) = oauth_server::wait_for_oauth_callback().await?;

        // Bot scopes for Lists API
        let scopes = "lists:read,lists:write";
        let url = format!(
            "{}?client_id={}&redirect_uri={}&scope={}&response_type=code",
            AUTH_URL,
            urlencoding(&config.client_id),
            urlencoding(&format!("http://127.0.0.1:{}/callback", port)),
            urlencoding(scopes),
        );

        Ok((url, port, rx))
    }

    /// Exchange an authorization code for a bot token.
    pub async fn exchange_code(&self, code: &str, port: u16) -> Result<(), String> {
        let (client_id, client_secret) = {
            let config = self.oauth_config.lock().await;
            let config = config.as_ref().ok_or("Slackが設定されていません")?;
            (config.client_id.clone(), config.client_secret.clone())
        };

        let resp = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("code", code),
                ("client_id", &client_id),
                ("client_secret", &client_secret),
                (
                    "redirect_uri",
                    &format!("http://127.0.0.1:{}/callback", port),
                ),
            ])
            .send()
            .await
            .map_err(|e| format!("Slack認証に失敗しました: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("応答の解析に失敗しました: {}", e))?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(format!("Slack認証エラー: {}", error));
        }

        let bot_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("Botトークンを取得できませんでした")?
            .to_string();
        let team_id = body
            .get("team")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let team_name = body
            .get("team")
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let tokens = SlackTokens {
            bot_token,
            team_id,
            team_name,
        };
        self.save_tokens(&tokens).await?;
        Ok(())
    }

    pub async fn logout(&self) -> Result<(), String> {
        let path = self.data_dir.join("slack_tokens.json");
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| format!("ログアウトに失敗しました: {}", e))?;
        }
        *self.tokens.lock().await = None;
        Ok(())
    }

    fn get_bot_token(&self, tokens: &SlackTokens) -> String {
        tokens.bot_token.clone()
    }

    /// Fetch items from a Slack List.
    pub async fn list_items(&self, list_id: &str) -> Result<Vec<SlackListItem>, String> {
        let tokens = self.tokens.lock().await;
        let tokens = tokens.as_ref().ok_or("Slackの認証が必要です")?;
        let token = self.get_bot_token(tokens);
        let _ = tokens;

        let resp = self
            .http
            .post("https://slack.com/api/slackLists.items.list")
            .bearer_auth(&token)
            .json(&serde_json::json!({ "list_id": list_id }))
            .send()
            .await
            .map_err(|e| format!("Slack APIエラー: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("応答の解析に失敗しました: {}", e))?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(format!("Slack APIエラー: {}", err));
        }

        let items = body
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let id = item.get("id")?.as_str()?.to_string();
                        let title = item
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("(無題)")
                            .to_string();
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

    /// Create a new item in a Slack List.
    pub async fn create_item(
        &self,
        list_id: &str,
        title: &str,
    ) -> Result<SlackListItem, String> {
        let tokens = self.tokens.lock().await;
        let tokens = tokens.as_ref().ok_or("Slackの認証が必要です")?;
        let token = self.get_bot_token(tokens);
        let _ = tokens;

        let resp = self
            .http
            .post("https://slack.com/api/slackLists.items.create")
            .bearer_auth(&token)
            .json(&serde_json::json!({
                "list_id": list_id,
                "item": { "title": title }
            }))
            .send()
            .await
            .map_err(|e| format!("Slack APIエラー: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("応答の解析に失敗しました: {}", e))?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            return Err(format!("Slack APIエラー: {}", err));
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

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("cowork-slack-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_slack_tokens_serialization() {
        let tokens = SlackTokens {
            bot_token: "xoxb-test".to_string(),
            team_id: Some("T123".to_string()),
            team_name: Some("テストチーム".to_string()),
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let parsed: SlackTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bot_token, "xoxb-test");
        assert_eq!(parsed.team_name, Some("テストチーム".to_string()));
    }

    #[test]
    fn test_slack_tokens_optional_fields() {
        let tokens = SlackTokens {
            bot_token: "xoxb-minimal".to_string(),
            team_id: None,
            team_name: None,
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let parsed: SlackTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.bot_token, "xoxb-minimal");
        assert!(parsed.team_id.is_none());
        assert!(parsed.team_name.is_none());
    }

    #[test]
    fn test_slack_oauth_config_serialization() {
        let config = SlackOAuthConfig {
            client_id: "123456.789012".to_string(),
            client_secret: "secret-value".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: SlackOAuthConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "123456.789012");
        assert_eq!(parsed.client_secret, "secret-value");
    }

    #[test]
    fn test_slack_settings_serialization() {
        let settings = SlackSettings {
            default_list_id: Some("L12345".to_string()),
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: SlackSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.default_list_id, Some("L12345".to_string()));
    }

    #[test]
    fn test_slack_settings_none() {
        let settings = SlackSettings {
            default_list_id: None,
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: SlackSettings = serde_json::from_str(&json).unwrap();
        assert!(parsed.default_list_id.is_none());
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
        assert!(!parsed.completed);
        assert_eq!(parsed.assignee, Some("U123".to_string()));
    }

    #[test]
    fn test_slack_list_item_minimal() {
        let item = SlackListItem {
            id: "item-2".to_string(),
            title: "最小限".to_string(),
            completed: true,
            assignee: None,
            due_date: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: SlackListItem = serde_json::from_str(&json).unwrap();
        assert!(parsed.completed);
        assert!(parsed.assignee.is_none());
        assert!(parsed.due_date.is_none());
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello"), "hello");
        assert_eq!(urlencoding("a b"), "a%20b");
        assert_eq!(urlencoding("foo@bar"), "foo%40bar");
        assert_eq!(
            urlencoding("lists:read,lists:write"),
            "lists%3Aread%2Clists%3Awrite"
        );
    }

    #[test]
    fn test_urlencoding_special_chars() {
        assert_eq!(urlencoding("a+b"), "a%2Bb");
        assert_eq!(urlencoding("a=b&c=d"), "a%3Db%26c%3Dd");
        assert_eq!(urlencoding("safe-chars_here.ok~"), "safe-chars_here.ok~");
    }

    #[tokio::test]
    async fn test_slack_client_not_configured() {
        let client = SlackClient::new(temp_dir(), None);
        assert!(!client.is_configured().await);
        assert!(!client.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_slack_client_load_empty_dir() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).await.unwrap();
        let client = SlackClient::new(dir.clone(), None);
        client.load().await.unwrap();
        assert!(!client.is_configured().await);
        assert!(!client.is_authenticated().await);
        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_slack_client_load_config_from_resource_dir() {
        let data_dir = temp_dir();
        let resource_dir = temp_dir();
        fs::create_dir_all(&resource_dir).await.unwrap();

        let config = SlackOAuthConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-secret".to_string(),
        };
        fs::write(
            resource_dir.join("slack_oauth.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .await
        .unwrap();

        let client = SlackClient::new(data_dir.clone(), Some(resource_dir.clone()));
        client.load().await.unwrap();
        assert!(client.is_configured().await);
        assert!(!client.is_authenticated().await);

        let _ = fs::remove_dir_all(&data_dir).await;
        let _ = fs::remove_dir_all(&resource_dir).await;
    }

    #[tokio::test]
    async fn test_slack_client_load_config_from_data_dir() {
        let data_dir = temp_dir();
        fs::create_dir_all(&data_dir).await.unwrap();

        let config = SlackOAuthConfig {
            client_id: "user-client-id".to_string(),
            client_secret: "user-secret".to_string(),
        };
        fs::write(
            data_dir.join("slack_oauth_config.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .await
        .unwrap();

        let client = SlackClient::new(data_dir.clone(), None);
        client.load().await.unwrap();
        assert!(client.is_configured().await);

        let _ = fs::remove_dir_all(&data_dir).await;
    }

    #[tokio::test]
    async fn test_slack_client_save_and_load_tokens() {
        let dir = temp_dir();
        let tokens = SlackTokens {
            bot_token: "xoxb-saved".to_string(),
            team_id: Some("T999".to_string()),
            team_name: Some("保存テスト".to_string()),
        };

        // Save tokens
        {
            let client = SlackClient::new(dir.clone(), None);
            client.save_tokens(&tokens).await.unwrap();
            assert!(client.is_authenticated().await);
            assert_eq!(client.get_team_name().await, Some("保存テスト".to_string()));
        }

        // Load from disk in new client
        {
            let client = SlackClient::new(dir.clone(), None);
            client.load().await.unwrap();
            assert!(client.is_authenticated().await);
            assert_eq!(client.get_team_name().await, Some("保存テスト".to_string()));
        }

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_slack_client_save_and_load_settings() {
        let dir = temp_dir();

        // Save settings
        {
            let client = SlackClient::new(dir.clone(), None);
            let settings = SlackSettings {
                default_list_id: Some("L99999".to_string()),
            };
            client.save_settings(settings).await.unwrap();
            let loaded = client.get_settings().await;
            assert_eq!(loaded.default_list_id, Some("L99999".to_string()));
        }

        // Load from disk in new client
        {
            let client = SlackClient::new(dir.clone(), None);
            client.load().await.unwrap();
            let loaded = client.get_settings().await;
            assert_eq!(loaded.default_list_id, Some("L99999".to_string()));
        }

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_slack_client_logout() {
        let dir = temp_dir();
        let client = SlackClient::new(dir.clone(), None);

        let tokens = SlackTokens {
            bot_token: "xoxb-logout-test".to_string(),
            team_id: None,
            team_name: None,
        };
        client.save_tokens(&tokens).await.unwrap();
        assert!(client.is_authenticated().await);

        client.logout().await.unwrap();
        assert!(!client.is_authenticated().await);
        assert!(client.get_team_name().await.is_none());

        // Token file should be deleted
        assert!(!dir.join("slack_tokens.json").exists());

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_slack_client_logout_no_tokens() {
        let client = SlackClient::new(temp_dir(), None);
        // Logout without prior auth should not error
        client.logout().await.unwrap();
        assert!(!client.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_slack_client_default_settings() {
        let client = SlackClient::new(temp_dir(), None);
        let settings = client.get_settings().await;
        assert!(settings.default_list_id.is_none());
    }

    #[tokio::test]
    async fn test_slack_client_get_team_name_not_authenticated() {
        let client = SlackClient::new(temp_dir(), None);
        assert!(client.get_team_name().await.is_none());
    }

    #[tokio::test]
    async fn test_slack_client_resource_dir_takes_precedence() {
        let data_dir = temp_dir();
        let resource_dir = temp_dir();
        fs::create_dir_all(&data_dir).await.unwrap();
        fs::create_dir_all(&resource_dir).await.unwrap();

        // Write config to both dirs
        let resource_config = SlackOAuthConfig {
            client_id: "resource-id".to_string(),
            client_secret: "resource-secret".to_string(),
        };
        fs::write(
            resource_dir.join("slack_oauth.json"),
            serde_json::to_string(&resource_config).unwrap(),
        )
        .await
        .unwrap();

        let data_config = SlackOAuthConfig {
            client_id: "data-id".to_string(),
            client_secret: "data-secret".to_string(),
        };
        fs::write(
            data_dir.join("slack_oauth_config.json"),
            serde_json::to_string(&data_config).unwrap(),
        )
        .await
        .unwrap();

        // Resource dir should take precedence
        let client = SlackClient::new(data_dir.clone(), Some(resource_dir.clone()));
        client.load().await.unwrap();
        assert!(client.is_configured().await);

        let _ = fs::remove_dir_all(&data_dir).await;
        let _ = fs::remove_dir_all(&resource_dir).await;
    }
}
