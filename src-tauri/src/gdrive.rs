use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Google Drive API v3 client with OAuth2 PKCE flow

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
    config_path: PathBuf,
    tokens_path: PathBuf,
    config: tokio::sync::Mutex<Option<GDriveConfig>>,
    tokens: tokio::sync::Mutex<Option<GDriveTokens>>,
}

impl GDriveClient {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            http: Client::new(),
            config_path: app_data_dir.join("gdrive_config.json"),
            tokens_path: app_data_dir.join("gdrive_tokens.json"),
            config: tokio::sync::Mutex::new(None),
            tokens: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn load(&self) -> Result<(), String> {
        if self.config_path.exists() {
            let content = fs::read_to_string(&self.config_path)
                .await
                .map_err(|e| format!("Failed to read gdrive config: {}", e))?;
            let cfg: GDriveConfig = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse gdrive config: {}", e))?;
            *self.config.lock().await = Some(cfg);
        }
        if self.tokens_path.exists() {
            let content = fs::read_to_string(&self.tokens_path)
                .await
                .map_err(|e| format!("Failed to read gdrive tokens: {}", e))?;
            let tokens: GDriveTokens = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse gdrive tokens: {}", e))?;
            *self.tokens.lock().await = Some(tokens);
        }
        Ok(())
    }

    pub async fn save_config(&self, config: GDriveConfig) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(&self.config_path, &content)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        *self.config.lock().await = Some(config);
        Ok(())
    }

    async fn save_tokens(&self, tokens: &GDriveTokens) -> Result<(), String> {
        if let Some(parent) = self.tokens_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("Failed to create dir: {}", e))?;
        }
        let content = serde_json::to_string_pretty(tokens)
            .map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(&self.tokens_path, &content)
            .await
            .map_err(|e| format!("Write error: {}", e))?;
        *self.tokens.lock().await = Some(tokens.clone());
        Ok(())
    }

    pub async fn is_configured(&self) -> bool {
        self.config.lock().await.is_some()
    }

    pub async fn is_authenticated(&self) -> bool {
        self.tokens.lock().await.is_some()
    }

    /// Generate OAuth2 authorization URL for user to visit
    pub async fn get_auth_url(&self, redirect_port: u16) -> Result<String, String> {
        let config = self.config.lock().await;
        let config = config.as_ref().ok_or("Google Drive not configured")?;

        let url = format!(
            "{}?client_id={}&redirect_uri=http://localhost:{}&response_type=code&scope={}&access_type=offline&prompt=consent",
            AUTH_URL,
            urlencoding(&config.client_id),
            redirect_port,
            urlencoding(SCOPES),
        );
        Ok(url)
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_port: u16,
    ) -> Result<(), String> {
        let (client_id, client_secret) = {
            let config = self.config.lock().await;
            let config = config.as_ref().ok_or("Google Drive not configured")?;
            (config.client_id.clone(), config.client_secret.clone())
        };

        let resp = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("code", code),
                ("client_id", &client_id),
                ("client_secret", &client_secret),
                ("redirect_uri", &format!("http://localhost:{}", redirect_port)),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| format!("Token exchange error: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
            return Err(format!("OAuth error: {}", err));
        }

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in response")?
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

    /// Refresh the access token using the refresh token
    pub async fn refresh_token(&self) -> Result<(), String> {
        let (client_id, client_secret) = {
            let config = self.config.lock().await;
            let config = config.as_ref().ok_or("Google Drive not configured")?;
            (config.client_id.clone(), config.client_secret.clone())
        };
        let refresh_tok = {
            let tokens = self.tokens.lock().await;
            let tokens = tokens.as_ref().ok_or("Not authenticated")?;
            tokens
                .refresh_token
                .clone()
                .ok_or("No refresh token available")?
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
            .map_err(|e| format!("Refresh error: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or("No access_token in refresh response")?
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
            let tokens = tokens.as_ref().ok_or("Not authenticated")?;
            (tokens.access_token.clone(), tokens.expires_at)
        };

        // Check if token is expired
        if let Some(exp) = expires_at {
            if chrono::Utc::now().timestamp() >= exp - 60 {
                self.refresh_token().await?;
                let tokens = self.tokens.lock().await;
                return Ok(tokens.as_ref().unwrap().access_token.clone());
            }
        }

        Ok(access_token)
    }

    /// List files in a Google Drive folder
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
            .map_err(|e| format!("Drive API error: {}", e))?;

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Parse error: {}", e))?;

        if let Some(err) = body.get("error") {
            return Err(format!("Drive error: {}", err));
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
                        let size = f.get("size").and_then(|v| v.as_str()).and_then(|s| s.parse().ok());
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

    /// Download a file to a local temporary path
    pub async fn download_file(&self, file_id: &str, dest: &str) -> Result<String, String> {
        let token = self.get_access_token().await?;
        let url = format!("{}/files/{}?alt=media", DRIVE_API, file_id);

        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("Download error: {}", e))?;

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Read error: {}", e))?;

        fs::write(dest, &bytes)
            .await
            .map_err(|e| format!("Write error: {}", e))?;

        Ok(dest.to_string())
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

    #[test]
    fn test_gdrive_config_serialization() {
        let config = GDriveConfig {
            client_id: "test-client-id".to_string(),
            client_secret: "test-secret".to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: GDriveConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.client_id, "test-client-id");
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
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello"), "hello");
        assert_eq!(urlencoding("a b"), "a%20b");
        assert_eq!(urlencoding("foo@bar"), "foo%40bar");
    }

    #[tokio::test]
    async fn test_gdrive_client_not_configured() {
        let dir = std::env::temp_dir().join(format!("cowork-gdrive-test-{}", uuid::Uuid::new_v4()));
        let client = GDriveClient::new(dir);
        assert!(!client.is_configured().await);
        assert!(!client.is_authenticated().await);
    }

    #[tokio::test]
    async fn test_gdrive_config_persistence() {
        let dir = std::env::temp_dir().join(format!("cowork-gdrive-test-{}", uuid::Uuid::new_v4()));
        let client = GDriveClient::new(dir.clone());
        client
            .save_config(GDriveConfig {
                client_id: "persist-test".to_string(),
                client_secret: "secret".to_string(),
            })
            .await
            .unwrap();

        let client2 = GDriveClient::new(dir.clone());
        client2.load().await.unwrap();
        assert!(client2.is_configured().await);

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_get_auth_url() {
        let dir = std::env::temp_dir().join(format!("cowork-gdrive-test-{}", uuid::Uuid::new_v4()));
        let client = GDriveClient::new(dir.clone());
        client
            .save_config(GDriveConfig {
                client_id: "my-client".to_string(),
                client_secret: "my-secret".to_string(),
            })
            .await
            .unwrap();

        let url = client.get_auth_url(8080).await.unwrap();
        assert!(url.contains("my-client"));
        assert!(url.contains("8080"));
        assert!(url.contains("accounts.google.com"));

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
