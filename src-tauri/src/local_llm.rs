use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::claude::ChatMessage;

/// Settings for local LLM via OpenAI-compatible API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalLlmSettings {
    pub enabled: bool,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub system_prompt: Option<String>,
}

impl Default for LocalLlmSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:8000/v1".to_string(),
            model: "default".to_string(),
            api_key: None,
            system_prompt: None,
        }
    }
}

/// OpenAI-compatible request/response types
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: Option<ChunkDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
}

/// Manager for local LLM interactions
pub struct LocalLlmManager {
    settings: Mutex<LocalLlmSettings>,
    conversation: Mutex<Vec<ApiMessage>>,
    data_dir: Mutex<Option<PathBuf>>,
}

impl LocalLlmManager {
    pub fn new() -> Self {
        Self {
            settings: Mutex::new(LocalLlmSettings::default()),
            conversation: Mutex::new(Vec::new()),
            data_dir: Mutex::new(None),
        }
    }

    pub async fn set_data_dir(&self, dir: PathBuf) {
        // Load saved settings
        let settings_file = dir.join("local_llm_settings.json");
        if settings_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&settings_file) {
                if let Ok(saved) = serde_json::from_str::<LocalLlmSettings>(&content) {
                    let mut s = self.settings.lock().await;
                    *s = saved;
                    log::info!("Loaded local LLM settings from disk");
                }
            }
        }
        let mut dd = self.data_dir.lock().await;
        *dd = Some(dir);
    }

    pub async fn get_settings(&self) -> LocalLlmSettings {
        self.settings.lock().await.clone()
    }

    pub async fn save_settings(&self, new_settings: LocalLlmSettings) -> Result<(), String> {
        let dd = self.data_dir.lock().await;
        if let Some(ref dir) = *dd {
            let content = serde_json::to_string_pretty(&new_settings)
                .map_err(|e| format!("設定のシリアライズに失敗: {}", e))?;
            std::fs::write(dir.join("local_llm_settings.json"), content)
                .map_err(|e| format!("設定の保存に失敗: {}", e))?;
        }
        let mut s = self.settings.lock().await;
        *s = new_settings;
        Ok(())
    }

    pub async fn clear_conversation(&self) {
        let mut conv = self.conversation.lock().await;
        conv.clear();
    }

    /// Send a message to the local LLM and stream the response
    pub async fn send_message(
        &self,
        app: &AppHandle,
        message: String,
    ) -> Result<(), String> {
        let settings = self.settings.lock().await.clone();

        if !settings.enabled {
            return Err("ローカルLLMが有効化されていません".to_string());
        }

        // Build conversation history
        {
            let mut conv = self.conversation.lock().await;
            conv.push(ApiMessage {
                role: "user".to_string(),
                content: message,
            });
        }

        // Prepare API messages (with optional system prompt)
        let mut api_messages: Vec<ApiMessage> = Vec::new();
        if let Some(ref sys) = settings.system_prompt {
            if !sys.is_empty() {
                api_messages.push(ApiMessage {
                    role: "system".to_string(),
                    content: sys.clone(),
                });
            }
        }
        {
            let conv = self.conversation.lock().await;
            api_messages.extend(conv.clone());
        }

        let request = ChatCompletionRequest {
            model: settings.model.clone(),
            messages: api_messages,
            stream: true,
        };

        let endpoint = settings.endpoint.trim_end_matches('/').to_string();
        let url = format!("{}/chat/completions", endpoint);

        let client = reqwest::Client::new();
        let mut req = client.post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream");

        if let Some(ref key) = settings.api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let response = req
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("ローカルLLMへの接続に失敗: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "ローカルLLMがエラーを返しました ({}): {}",
                status, body
            ));
        }

        // Parse SSE stream
        let mut full_response = String::new();
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;
        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| format!("ストリーム読み取りエラー: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE lines
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || line == "data: [DONE]" {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                        for choice in &chunk.choices {
                            if let Some(ref delta) = choice.delta {
                                if let Some(ref content) = delta.content {
                                    full_response.push_str(content);
                                    let _ = app.emit("claude:text_delta", content.as_str());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Process any remaining buffer
        for line in buffer.lines() {
            let line = line.trim();
            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }
            if let Some(data) = line.strip_prefix("data: ") {
                if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                    for choice in &chunk.choices {
                        if let Some(ref delta) = choice.delta {
                            if let Some(ref content) = delta.content {
                                full_response.push_str(content);
                                let _ = app.emit("claude:text_delta", content.as_str());
                            }
                        }
                    }
                }
            }
        }

        // Emit final message
        if !full_response.is_empty() {
            let msg = ChatMessage {
                id: uuid::Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                content: full_response.clone(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };
            let _ = app.emit("claude:message", &msg);
        }

        // Add assistant response to conversation history
        {
            let mut conv = self.conversation.lock().await;
            conv.push(ApiMessage {
                role: "assistant".to_string(),
                content: full_response,
            });
        }

        // Signal completion
        let _ = app.emit("claude:done", true);

        Ok(())
    }

    /// Test connectivity to the configured endpoint
    pub async fn test_connection(&self) -> Result<String, String> {
        let settings = self.settings.lock().await.clone();
        let endpoint = settings.endpoint.trim_end_matches('/').to_string();
        let url = format!("{}/models", endpoint);

        let client = reqwest::Client::new();
        let mut req = client.get(&url);

        if let Some(ref key) = settings.api_key {
            if !key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
        }

        let response = req
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| format!("接続テスト失敗: {}", e))?;

        if response.status().is_success() {
            Ok("接続成功".to_string())
        } else {
            Err(format!("サーバーエラー: {}", response.status()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = LocalLlmSettings::default();
        assert!(!settings.enabled);
        assert_eq!(settings.endpoint, "http://localhost:8000/v1");
        assert_eq!(settings.model, "default");
        assert!(settings.api_key.is_none());
    }

    #[test]
    fn test_settings_serialize_roundtrip() {
        let settings = LocalLlmSettings {
            enabled: true,
            endpoint: "http://localhost:11434/v1".to_string(),
            model: "llama3".to_string(),
            api_key: Some("test-key".to_string()),
            system_prompt: Some("あなたは親切なアシスタントです".to_string()),
        };
        let json = serde_json::to_string(&settings).unwrap();
        let restored: LocalLlmSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.enabled, true);
        assert_eq!(restored.endpoint, "http://localhost:11434/v1");
        assert_eq!(restored.model, "llama3");
        assert_eq!(restored.api_key.unwrap(), "test-key");
    }

    #[test]
    fn test_parse_sse_chunk() {
        let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(data).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(
            chunk.choices[0].delta.as_ref().unwrap().content.as_ref().unwrap(),
            "Hello"
        );
    }

    #[test]
    fn test_parse_sse_chunk_finish() {
        let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(data).unwrap();
        assert_eq!(chunk.choices[0].finish_reason.as_ref().unwrap(), "stop");
    }

    #[tokio::test]
    async fn test_manager_default_settings() {
        let mgr = LocalLlmManager::new();
        let settings = mgr.get_settings().await;
        assert!(!settings.enabled);
    }

    #[tokio::test]
    async fn test_manager_save_and_load_settings() {
        let mgr = LocalLlmManager::new();
        let new_settings = LocalLlmSettings {
            enabled: true,
            endpoint: "http://localhost:9999/v1".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            system_prompt: None,
        };
        // Save without data_dir set - should still update in-memory
        let _ = mgr.save_settings(new_settings).await;
        let loaded = mgr.get_settings().await;
        assert!(loaded.enabled);
        assert_eq!(loaded.model, "test-model");
    }

    #[tokio::test]
    async fn test_manager_clear_conversation() {
        let mgr = LocalLlmManager::new();
        {
            let mut conv = mgr.conversation.lock().await;
            conv.push(ApiMessage {
                role: "user".to_string(),
                content: "test".to_string(),
            });
        }
        mgr.clear_conversation().await;
        let conv = mgr.conversation.lock().await;
        assert!(conv.is_empty());
    }
}
