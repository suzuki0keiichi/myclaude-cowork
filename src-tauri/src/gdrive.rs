use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

use crate::oauth_server;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";
const SCOPES: &str = "https://www.googleapis.com/auth/drive";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDriveConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GDriveTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub is_folder: bool,
    pub size: Option<u64>,
    pub modified_time: Option<String>,
}

pub struct GDriveClient {
    http: Client,
    data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    config: tokio::sync::Mutex<Option<GDriveConfig>>,
    tokens: tokio::sync::Mutex<Option<GDriveTokens>>,
}

impl GDriveClient {
    pub fn new(data_dir: PathBuf, resource_dir: Option<PathBuf>) -> Self {
        Self {
            http: Client::new(),
            data_dir,
            resource_dir,
            config: tokio::sync::Mutex::new(None),
            tokens: tokio::sync::Mutex::new(None),
        }
    }

    /// Load config from bundled resources or user data dir, and tokens from data dir.
    pub async fn load(&self) -> Result<(), String> {
        // Load OAuth config: bundled resource first, then user data dir
        if let Some(config) = self.load_config().await {
            *self.config.lock().await = Some(config);
        }

        // Load tokens from data dir
        let tokens_path = self.data_dir.join("gdrive_tokens.json");
        if tokens_path.exists() {
            let content = fs::read_to_string(&tokens_path)
                .await
                .map_err(|e| format!("Google Drive認証情報を読み込めませんでした: {}", e))?;
            let tokens: GDriveTokens = serde_json::from_str(&content)
                .map_err(|e| format!("Google Drive認証情報の形式が正しくありません: {}", e))?;
            *self.tokens.lock().await = Some(tokens);
        }

        Ok(())
    }

    async fn load_config(&self) -> Option<GDriveConfig> {
        // 1. Try bundled resource
        if let Some(ref dir) = self.resource_dir {
            let path = dir.join("gdrive_oauth.json");
            if let Ok(content) = fs::read_to_string(&path).await {
                if let Ok(config) = serde_json::from_str::<GDriveConfig>(&content) {
                    log::info!("Google Drive OAuth config loaded from resources: {}", path.display());
                    return Some(config);
                }
            }
        }

        // 2. Try user data dir (backward compat / manual config)
        let path = self.data_dir.join("gdrive_config.json");
        if let Ok(content) = fs::read_to_string(&path).await {
            if let Ok(config) = serde_json::from_str::<GDriveConfig>(&content) {
                log::info!("Google Drive OAuth config loaded from data dir");
                return Some(config);
            }
        }

        None
    }

    async fn save_tokens(&self, tokens: &GDriveTokens) -> Result<(), String> {
        fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| format!("フォルダを作成できませんでした: {}", e))?;
        let path = self.data_dir.join("gdrive_tokens.json");
        let content = serde_json::to_string_pretty(tokens)
            .map_err(|e| format!("データの変換に失敗しました: {}", e))?;
        fs::write(&path, &content)
            .await
            .map_err(|e| format!("ファイルの書き込みに失敗しました: {}", e))?;
        *self.tokens.lock().await = Some(tokens.clone());
        Ok(())
    }

    pub async fn is_configured(&self) -> bool {
        self.config.lock().await.is_some()
    }

    pub async fn is_authenticated(&self) -> bool {
        self.tokens.lock().await.is_some()
    }

    /// Start the OAuth flow: spawn a callback server and return the auth URL.
    /// The caller should open the URL in a browser, then await the receiver
    /// for the auth code.
    pub async fn start_auth_flow(
        &self,
    ) -> Result<(String, u16, tokio::sync::oneshot::Receiver<Result<String, String>>), String> {
        let config = self.config.lock().await;
        let config = config
            .as_ref()
            .ok_or("Google DriveのOAuth設定が見つかりません。開発者に連絡してください。")?;

        let (port, rx) = oauth_server::wait_for_oauth_callback().await?;

        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
            AUTH_URL,
            urlencoding(&config.client_id),
            urlencoding(&format!("http://127.0.0.1:{}/callback", port)),
            urlencoding(SCOPES),
        );

        Ok((url, port, rx))
    }

    /// Exchange an authorization code for tokens.
    pub async fn exchange_code(&self, code: &str, port: u16) -> Result<(), String> {
        let (client_id, client_secret) = {
            let config = self.config.lock().await;
            let config = config
                .as_ref()
                .ok_or("Google Driveが設定されていません")?;
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
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("認証トークンの取得に失敗しました: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("応答の解析に失敗しました: {}", e))?;

        if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
            let desc = body
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            return Err(format!("OAuth認証エラー: {} {}", err, desc));
        }

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("認証トークンを取得できませんでした")?
            .to_string();
        let refresh_token = body
            .get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let expires_in = body.get("expires_in").and_then(|v| v.as_i64());
        let expires_at = expires_in.map(|e| chrono::Utc::now().timestamp() + e);

        let tokens = GDriveTokens {
            access_token,
            refresh_token,
            expires_at,
        };
        self.save_tokens(&tokens).await?;
        Ok(())
    }

    async fn refresh_token(&self) -> Result<(), String> {
        let (client_id, client_secret) = {
            let config = self.config.lock().await;
            let config = config
                .as_ref()
                .ok_or("Google Driveが設定されていません")?;
            (config.client_id.clone(), config.client_secret.clone())
        };
        let refresh_tok = {
            let tokens = self.tokens.lock().await;
            let tokens = tokens
                .as_ref()
                .ok_or("Google Driveの認証が必要です")?;
            tokens
                .refresh_token
                .clone()
                .ok_or("再認証が必要です")?
        };

        let resp = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("refresh_token", refresh_tok.as_str()),
                ("client_id", &client_id),
                ("client_secret", &client_secret),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| format!("認証の更新に失敗しました: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("応答の解析に失敗しました: {}", e))?;

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("認証トークンの更新に失敗しました")?
            .to_string();
        let expires_in = body.get("expires_in").and_then(|v| v.as_i64());
        let expires_at = expires_in.map(|e| chrono::Utc::now().timestamp() + e);

        let new_tokens = GDriveTokens {
            access_token,
            refresh_token: Some(refresh_tok),
            expires_at,
        };
        self.save_tokens(&new_tokens).await?;
        Ok(())
    }

    async fn get_access_token(&self) -> Result<String, String> {
        let (access_token, expires_at) = {
            let tokens = self.tokens.lock().await;
            let tokens = tokens
                .as_ref()
                .ok_or("Google Driveの認証が必要です")?;
            (tokens.access_token.clone(), tokens.expires_at)
        };

        if let Some(exp) = expires_at {
            if chrono::Utc::now().timestamp() >= exp - 60 {
                self.refresh_token().await?;
                let tokens = self.tokens.lock().await;
                return Ok(tokens.as_ref().unwrap().access_token.clone());
            }
        }

        Ok(access_token)
    }

    /// List files in a Google Drive folder.
    pub async fn list_files(&self, folder_id: Option<&str>) -> Result<Vec<DriveFile>, String> {
        let token = self.get_access_token().await?;
        let parent = folder_id.unwrap_or("root");

        let query = format!("'{}' in parents and trashed = false", parent);
        let url = format!(
            "{}/files?q={}&fields=files(id,name,mimeType,size,modifiedTime)&orderBy=folder,name&pageSize=100",
            DRIVE_API,
            urlencoding(&query),
        );

        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("Google Drive APIエラー: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("応答の解析に失敗しました: {}", e))?;

        if let Some(err) = body.get("error") {
            return Err(format!("Google Driveエラー: {}", err));
        }

        let files = body
            .get("files")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|f| {
                        let id = f.get("id")?.as_str()?.to_string();
                        let name = f.get("name")?.as_str()?.to_string();
                        let mime = f
                            .get("mimeType")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_folder = mime == "application/vnd.google-apps.folder";
                        let size = f
                            .get("size")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok());
                        let modified = f
                            .get("modifiedTime")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        Some(DriveFile {
                            id,
                            name,
                            mime_type: mime,
                            is_folder,
                            size,
                            modified_time: modified,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(files)
    }

    /// Download a file to a local path.
    pub async fn download_file(&self, file_id: &str, dest: &str) -> Result<String, String> {
        let token = self.get_access_token().await?;
        let url = format!("{}/files/{}?alt=media", DRIVE_API, file_id);

        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("ダウンロードに失敗しました: {}", e))?;

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("ファイルの読み込みに失敗しました: {}", e))?;

        fs::write(dest, &bytes)
            .await
            .map_err(|e| format!("ファイルの書き込みに失敗しました: {}", e))?;

        Ok(dest.to_string())
    }

    /// Clear tokens (logout).
    pub async fn logout(&self) -> Result<(), String> {
        let path = self.data_dir.join("gdrive_tokens.json");
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| format!("ログアウトに失敗しました: {}", e))?;
        }
        *self.tokens.lock().await = None;
        Ok(())
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
        std::env::temp_dir().join(format!("cowork-gdrive-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_gdrive_config_serialization() {
        let config = GDriveConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-secret".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: GDriveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "test-client-id");
        assert_eq!(parsed.client_secret, "test-secret");
    }

    #[test]
    fn test_gdrive_tokens_serialization() {
        let tokens = GDriveTokens {
            access_token: "ya29.access".to_string(),
            refresh_token: Some("1//refresh".to_string()),
            expires_at: Some(1700000000),
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let parsed: GDriveTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "ya29.access");
        assert_eq!(parsed.refresh_token, Some("1//refresh".to_string()));
        assert_eq!(parsed.expires_at, Some(1700000000));
    }

    #[test]
    fn test_gdrive_tokens_optional_fields() {
        let tokens = GDriveTokens {
            access_token: "ya29.minimal".to_string(),
            refresh_token: None,
            expires_at: None,
        };
        let json = serde_json::to_string(&tokens).unwrap();
        let parsed: GDriveTokens = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.access_token, "ya29.minimal");
        assert!(parsed.refresh_token.is_none());
        assert!(parsed.expires_at.is_none());
    }

    #[test]
    fn test_drive_file_serialization() {
        let file = DriveFile {
            id: "file-1".to_string(),
            name: "レポート.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            is_folder: false,
            size: Some(1024),
            modified_time: Some("2026-02-07T00:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&file).unwrap();
        let parsed: DriveFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "レポート.pdf");
        assert_eq!(parsed.mime_type, "application/pdf");
        assert!(!parsed.is_folder);
        assert_eq!(parsed.size, Some(1024));
    }

    #[test]
    fn test_drive_file_folder() {
        let folder = DriveFile {
            id: "folder-1".to_string(),
            name: "書類".to_string(),
            mime_type: "application/vnd.google-apps.folder".to_string(),
            is_folder: true,
            size: None,
            modified_time: None,
        };
        let json = serde_json::to_string(&folder).unwrap();
        let parsed: DriveFile = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_folder);
        assert!(parsed.size.is_none());
        assert!(parsed.modified_time.is_none());
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello"), "hello");
        assert_eq!(urlencoding("a b"), "a%20b");
        assert_eq!(urlencoding("foo@bar"), "foo%40bar");
    }

    #[test]
    fn test_urlencoding_special_chars() {
        assert_eq!(urlencoding("a+b"), "a%2Bb");
        assert_eq!(urlencoding("a=b&c=d"), "a%3Db%26c%3Dd");
        assert_eq!(urlencoding("/path/to"), "%2Fpath%2Fto");
        assert_eq!(urlencoding("safe-chars_here.ok~"), "safe-chars_here.ok~");
    }

    #[test]
    fn test_urlencoding_scope() {
        let scope = "https://www.googleapis.com/auth/drive";
        let encoded = urlencoding(scope);
        assert!(encoded.contains("https%3A%2F%2F"));
        assert!(encoded.contains("drive"));
    }

    #[tokio::test]
    async fn test_gdrive_client_not_configured() {
        let client = GDriveClient::new(temp_dir(), None);
        assert!(!client.is_configured().await);
        assert!(!client.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_gdrive_client_load_empty_dir() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).await.unwrap();
        let client = GDriveClient::new(dir.clone(), None);
        client.load().await.unwrap();
        assert!(!client.is_configured().await);
        assert!(!client.is_authenticated().await);
        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_gdrive_client_load_config_from_resource_dir() {
        let data_dir = temp_dir();
        let resource_dir = temp_dir();
        fs::create_dir_all(&resource_dir).await.unwrap();

        let config = GDriveConfig {
            client_id: "resource-client-id".to_string(),
            client_secret: "resource-secret".to_string(),
        };
        fs::write(
            resource_dir.join("gdrive_oauth.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .await
        .unwrap();

        let client = GDriveClient::new(data_dir.clone(), Some(resource_dir.clone()));
        client.load().await.unwrap();
        assert!(client.is_configured().await);
        assert!(!client.is_authenticated().await);

        let _ = fs::remove_dir_all(&data_dir).await;
        let _ = fs::remove_dir_all(&resource_dir).await;
    }

    #[tokio::test]
    async fn test_gdrive_client_load_config_from_data_dir() {
        let data_dir = temp_dir();
        fs::create_dir_all(&data_dir).await.unwrap();

        let config = GDriveConfig {
            client_id: "user-client-id".to_string(),
            client_secret: "user-secret".to_string(),
        };
        fs::write(
            data_dir.join("gdrive_config.json"),
            serde_json::to_string(&config).unwrap(),
        )
        .await
        .unwrap();

        let client = GDriveClient::new(data_dir.clone(), None);
        client.load().await.unwrap();
        assert!(client.is_configured().await);

        let _ = fs::remove_dir_all(&data_dir).await;
    }

    #[tokio::test]
    async fn test_gdrive_client_save_and_load_tokens() {
        let dir = temp_dir();
        let tokens = GDriveTokens {
            access_token: "ya29.saved".to_string(),
            refresh_token: Some("1//saved-refresh".to_string()),
            expires_at: Some(9999999999),
        };

        // Save tokens
        {
            let client = GDriveClient::new(dir.clone(), None);
            client.save_tokens(&tokens).await.unwrap();
            assert!(client.is_authenticated().await);
        }

        // Load from disk in new client
        {
            let client = GDriveClient::new(dir.clone(), None);
            client.load().await.unwrap();
            assert!(client.is_authenticated().await);
        }

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_gdrive_client_logout() {
        let dir = temp_dir();
        let client = GDriveClient::new(dir.clone(), None);

        let tokens = GDriveTokens {
            access_token: "ya29.logout-test".to_string(),
            refresh_token: None,
            expires_at: None,
        };
        client.save_tokens(&tokens).await.unwrap();
        assert!(client.is_authenticated().await);

        client.logout().await.unwrap();
        assert!(!client.is_authenticated().await);

        // Token file should be deleted
        assert!(!dir.join("gdrive_tokens.json").exists());

        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_gdrive_client_logout_no_tokens() {
        let client = GDriveClient::new(temp_dir(), None);
        // Logout without prior auth should not error
        client.logout().await.unwrap();
        assert!(!client.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_gdrive_client_resource_dir_takes_precedence() {
        let data_dir = temp_dir();
        let resource_dir = temp_dir();
        fs::create_dir_all(&data_dir).await.unwrap();
        fs::create_dir_all(&resource_dir).await.unwrap();

        // Write config to both dirs
        let resource_config = GDriveConfig {
            client_id: "resource-id".to_string(),
            client_secret: "resource-secret".to_string(),
        };
        fs::write(
            resource_dir.join("gdrive_oauth.json"),
            serde_json::to_string(&resource_config).unwrap(),
        )
        .await
        .unwrap();

        let data_config = GDriveConfig {
            client_id: "data-id".to_string(),
            client_secret: "data-secret".to_string(),
        };
        fs::write(
            data_dir.join("gdrive_config.json"),
            serde_json::to_string(&data_config).unwrap(),
        )
        .await
        .unwrap();

        // Resource dir config should take precedence
        let client = GDriveClient::new(data_dir.clone(), Some(resource_dir.clone()));
        client.load().await.unwrap();
        assert!(client.is_configured().await);

        let _ = fs::remove_dir_all(&data_dir).await;
        let _ = fs::remove_dir_all(&resource_dir).await;
    }

    #[tokio::test]
    async fn test_gdrive_client_start_auth_fails_without_config() {
        let client = GDriveClient::new(temp_dir(), None);
        let result = client.start_auth_flow().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OAuth設定が見つかりません"));
    }
}
