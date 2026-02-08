use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::approval_server;
use crate::translator::translate_tool_event;

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// A single message in the chat history (sent to frontend)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String, // "user" | "assistant" | "system"
    pub content: String,
    pub timestamp: String,
}

/// Activity item shown in the activity panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityItem {
    pub id: String,
    pub description: String,       // Human-readable Japanese description
    pub raw_command: Option<String>, // Original command (for debug toggle)
    pub status: String,            // "running" | "done" | "error"
    pub timestamp: String,
}

/// Stream event types from Claude Code
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeStreamEvent {
    #[serde(rename = "system")]
    System {
        subtype: Option<String>,
        session_id: Option<String>,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    #[serde(rename = "assistant")]
    Assistant {
        message: AssistantMessage,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    #[serde(rename = "user")]
    User {
        message: serde_json::Value,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    #[serde(rename = "result")]
    Result {
        subtype: Option<String>,
        result: Option<String>,
        is_error: Option<bool>,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
    #[serde(rename = "stream_event")]
    StreamEvent {
        event: serde_json::Value,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Option<serde_json::Value>,
        #[serde(flatten)]
        extra: serde_json::Value,
    },
}

pub struct ClaudeManager {
    session_id: Arc<Mutex<Option<String>>>,
    working_dir: Mutex<String>,
    approval_port: Arc<Mutex<Option<u16>>>,
    approval_pending: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl ClaudeManager {
    pub fn new(
        approval_pending: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    ) -> Self {
        Self {
            session_id: Arc::new(Mutex::new(None)),
            working_dir: Mutex::new(String::new()),
            approval_port: Arc::new(Mutex::new(None)),
            approval_pending,
        }
    }

    pub async fn get_working_dir(&self) -> String {
        self.working_dir.lock().await.clone()
    }

    pub async fn set_working_dir(&self, dir: String) {
        let mut wd = self.working_dir.lock().await;
        *wd = dir;
    }

    pub async fn reset_session(&self) {
        let mut s = self.session_id.lock().await;
        *s = None;
    }

    /// Ensure the approval server is running, return port
    async fn ensure_approval_server(&self, app: &AppHandle) -> Result<u16, String> {
        let mut port_guard = self.approval_port.lock().await;
        if let Some(port) = *port_guard {
            return Ok(port);
        }
        let port = approval_server::start_approval_server(
            app.clone(),
            Arc::clone(&self.approval_pending),
        ).await?;
        *port_guard = Some(port);
        Ok(port)
    }

    /// Install the hook script and configure Claude Code settings
    pub fn ensure_hook_installed(app: &AppHandle) -> Result<PathBuf, String> {
        let data_dir = app.path().app_data_dir()
            .map_err(|e| format!("アプリデータディレクトリ取得エラー: {}", e))?;
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| format!("ディレクトリ作成エラー: {}", e))?;

        // Copy hook script to app data directory
        let hook_dest = data_dir.join("cowork-hook.cjs");
        let hook_source = include_str!("../resources/cowork-hook.cjs");
        std::fs::write(&hook_dest, hook_source)
            .map_err(|e| format!("hookスクリプト書き込みエラー: {}", e))?;

        // Configure Claude Code settings
        let home = home_dir().ok_or("ホームディレクトリが見つかりません")?;
        let claude_dir = home.join(".claude");
        let settings_path = claude_dir.join("settings.json");

        // Read existing settings or create new
        let mut settings: serde_json::Value = if settings_path.exists() {
            let content = std::fs::read_to_string(&settings_path)
                .map_err(|e| format!("settings.json読み込みエラー: {}", e))?;
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            std::fs::create_dir_all(&claude_dir)
                .map_err(|e| format!(".claudeディレクトリ作成エラー: {}", e))?;
            serde_json::json!({})
        };

        // Build the hook command
        let hook_command = format!("node {}", hook_dest.to_string_lossy().replace('\\', "\\\\"));

        // Check if hook is already configured
        let already_configured = settings.get("hooks")
            .and_then(|h| h.get("PreToolUse"))
            .and_then(|p| p.as_array())
            .map(|arr| arr.iter().any(|item| {
                item.get("hooks")
                    .and_then(|h| h.as_array())
                    .map(|hooks| hooks.iter().any(|hook| {
                        hook.get("command")
                            .and_then(|c| c.as_str())
                            .map(|c| c.contains("cowork-hook"))
                            .unwrap_or(false)
                    }))
                    .unwrap_or(false)
            }))
            .unwrap_or(false);

        if !already_configured {
            // Backup existing settings
            if settings_path.exists() {
                let backup_path = claude_dir.join("settings.json.cowork-backup");
                let _ = std::fs::copy(&settings_path, &backup_path);
            }

            // Add hook configuration
            let hook_config = serde_json::json!({
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": hook_command
                }]
            });

            let hooks = settings.as_object_mut().unwrap()
                .entry("hooks")
                .or_insert_with(|| serde_json::json!({}));
            let pre_tool_use = hooks.as_object_mut().unwrap()
                .entry("PreToolUse")
                .or_insert_with(|| serde_json::json!([]));

            if let Some(arr) = pre_tool_use.as_array_mut() {
                arr.push(hook_config);
            }

            std::fs::write(&settings_path, serde_json::to_string_pretty(&settings).unwrap())
                .map_err(|e| format!("settings.json書き込みエラー: {}", e))?;

            log::info!("Cowork hook installed in {}", settings_path.display());
        }

        Ok(hook_dest)
    }

    /// Send a user message to Claude Code and stream the response
    pub async fn send_message(
        &self,
        app: &AppHandle,
        message: String,
    ) -> Result<(), String> {
        let working_dir = self.working_dir.lock().await.clone();
        if working_dir.is_empty() {
            return Err("作業フォルダが設定されていません".to_string());
        }

        // Ensure approval server is running
        let approval_port = self.ensure_approval_server(app).await?;

        // Build command args
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
        ];

        // If we have a session, continue it
        let session = self.session_id.lock().await.clone();
        if let Some(sid) = &session {
            args.push("--resume".to_string());
            args.push(sid.clone());
        }

        args.push(message);

        log::info!("Spawning claude with args: {:?}, approval_port: {}", args, approval_port);

        // Spawn claude process with approval port environment variable
        let mut child = Command::new("claude")
            .args(&args)
            .current_dir(&working_dir)
            .env("COWORK_APPROVAL_PORT", approval_port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Claude Codeを起動できませんでした: {}", e))?;

        let stdout = child.stdout.take().ok_or("Claude Codeの出力を取得できませんでした")?;
        let stderr = child.stderr.take().ok_or("Claude Codeのエラー出力を取得できませんでした")?;

        // Read stdout line by line (NDJSON)
        let app_handle = app.clone();
        let session_id_ref = Arc::clone(&self.session_id);

        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut current_text = String::new();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }

                let parsed: Result<ClaudeStreamEvent, _> = serde_json::from_str(&line);

                match parsed {
                    Ok(event) => {
                        match &event {
                            ClaudeStreamEvent::System { session_id, .. } => {
                                if let Some(sid) = session_id {
                                    let mut s = session_id_ref.lock().await;
                                    *s = Some(sid.clone());
                                }
                                let _ = app_handle.emit("claude:system", &event);
                            }

                            ClaudeStreamEvent::Assistant { message, .. } => {
                                // Process content blocks
                                for block in &message.content {
                                    match block {
                                        ContentBlock::Text { text } => {
                                            current_text = text.clone();
                                            let msg = ChatMessage {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                role: "assistant".to_string(),
                                                content: text.clone(),
                                                timestamp: chrono::Utc::now().to_rfc3339(),
                                            };
                                            let _ = app_handle.emit("claude:message", &msg);
                                        }
                                        ContentBlock::ToolUse { id, name, input } => {
                                            let translated = translate_tool_event(name, input);
                                            let activity = ActivityItem {
                                                id: id.clone(),
                                                description: translated.description,
                                                raw_command: Some(translated.raw),
                                                status: "running".to_string(),
                                                timestamp: chrono::Utc::now().to_rfc3339(),
                                            };
                                            let _ = app_handle.emit("claude:activity", &activity);
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            ClaudeStreamEvent::User { message, .. } => {
                                // Tool results - mark activity as done
                                if let Some(content) = message.get("content") {
                                    if let Some(arr) = content.as_array() {
                                        for item in arr {
                                            if let Some(tool_id) = item.get("tool_use_id").and_then(|v| v.as_str()) {
                                                let activity = ActivityItem {
                                                    id: tool_id.to_string(),
                                                    description: "完了".to_string(),
                                                    raw_command: None,
                                                    status: "done".to_string(),
                                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                                };
                                                let _ = app_handle.emit("claude:activity_done", &activity);
                                            }
                                        }
                                    }
                                }
                            }

                            ClaudeStreamEvent::Result { result, .. } => {
                                let _ = app_handle.emit("claude:result", &event);

                                if let Some(text) = result {
                                    if !text.is_empty() && text != &current_text {
                                        let msg = ChatMessage {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            role: "assistant".to_string(),
                                            content: text.clone(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        };
                                        let _ = app_handle.emit("claude:message", &msg);
                                    }
                                }
                            }

                            ClaudeStreamEvent::StreamEvent { event: evt, .. } => {
                                // Forward text deltas for real-time streaming
                                if let Some(delta) = evt.get("delta") {
                                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                        let _ = app_handle.emit("claude:text_delta", text);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to parse stream line: {} - line: {}", e, &line[..line.len().min(200)]);
                    }
                }
            }
        });

        // Read stderr for errors
        let app_handle2 = app.clone();
        let stderr_task = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    log::warn!("claude stderr: {}", line);
                    let _ = app_handle2.emit("claude:stderr", &line);
                }
            }
        });

        // Wait for process to finish
        let status = child.wait().await.map_err(|e| format!("プロセスエラー: {}", e))?;
        let _ = stdout_task.await;
        let _ = stderr_task.await;

        // Signal completion to frontend
        let _ = app.emit("claude:done", status.success());

        if !status.success() {
            log::error!("Claude process exited with status: {}", status);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // ── ClaudeStreamEvent deserialization ──

    #[test]
    fn test_parse_system_event() {
        let json_str = r#"{"type":"system","subtype":"init","session_id":"abc-123","cwd":"/tmp"}"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::System { subtype, session_id, .. } => {
                assert_eq!(subtype.unwrap(), "init");
                assert_eq!(session_id.unwrap(), "abc-123");
            }
            _ => panic!("Expected System event"),
        }
    }

    #[test]
    fn test_parse_assistant_text() {
        let json_str = r#"{
            "type": "assistant",
            "message": {
                "id": "msg_01",
                "role": "assistant",
                "model": "claude-sonnet-4-5-20250929",
                "content": [{"type": "text", "text": "Hello!"}]
            }
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::Assistant { message, .. } => {
                assert_eq!(message.content.len(), 1);
                match &message.content[0] {
                    ContentBlock::Text { text } => assert_eq!(text, "Hello!"),
                    _ => panic!("Expected Text block"),
                }
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_assistant_tool_use() {
        let json_str = r#"{
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "Let me check."},
                    {"type": "tool_use", "id": "toolu_01", "name": "Read", "input": {"file_path": "/tmp/x"}}
                ]
            }
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::Assistant { message, .. } => {
                assert_eq!(message.content.len(), 2);
                match &message.content[1] {
                    ContentBlock::ToolUse { id, name, input } => {
                        assert_eq!(id, "toolu_01");
                        assert_eq!(name, "Read");
                        assert_eq!(input["file_path"], "/tmp/x");
                    }
                    _ => panic!("Expected ToolUse block"),
                }
            }
            _ => panic!("Expected Assistant event"),
        }
    }

    #[test]
    fn test_parse_user_tool_result() {
        let json_str = r#"{
            "type": "user",
            "message": {
                "role": "user",
                "content": [{"type": "tool_result", "tool_use_id": "toolu_01", "content": "file data"}]
            }
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::User { message, .. } => {
                let content = message["content"].as_array().unwrap();
                assert_eq!(content[0]["tool_use_id"], "toolu_01");
            }
            _ => panic!("Expected User event"),
        }
    }

    #[test]
    fn test_parse_result_success() {
        let json_str = r#"{
            "type": "result",
            "subtype": "success",
            "result": "Done!",
            "is_error": false,
            "duration_ms": 1234,
            "num_turns": 2,
            "total_cost_usd": 0.01
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::Result { subtype, result, is_error, .. } => {
                assert_eq!(subtype.unwrap(), "success");
                assert_eq!(result.unwrap(), "Done!");
                assert_eq!(is_error.unwrap(), false);
            }
            _ => panic!("Expected Result event"),
        }
    }

    #[test]
    fn test_parse_result_error() {
        let json_str = r#"{
            "type": "result",
            "subtype": "error_max_turns",
            "result": "",
            "is_error": true
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::Result { subtype, is_error, .. } => {
                assert_eq!(subtype.unwrap(), "error_max_turns");
                assert_eq!(is_error.unwrap(), true);
            }
            _ => panic!("Expected Result event"),
        }
    }

    #[test]
    fn test_parse_stream_event_text_delta() {
        let json_str = r#"{
            "type": "stream_event",
            "event": {
                "type": "content_block_delta",
                "index": 0,
                "delta": {"type": "text_delta", "text": "He"}
            }
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        match event {
            ClaudeStreamEvent::StreamEvent { event, .. } => {
                let text = event["delta"]["text"].as_str().unwrap();
                assert_eq!(text, "He");
            }
            _ => panic!("Expected StreamEvent"),
        }
    }

    // ── ContentBlock ──

    #[test]
    fn test_content_block_text() {
        let json_str = r#"{"type": "text", "text": "hello world"}"#;
        let block: ContentBlock = serde_json::from_str(json_str).unwrap();
        match block {
            ContentBlock::Text { text } => assert_eq!(text, "hello world"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_content_block_tool_use() {
        let json_str = r#"{"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": "ls"}}"#;
        let block: ContentBlock = serde_json::from_str(json_str).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "t1");
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls");
            }
            _ => panic!("Expected ToolUse"),
        }
    }

    // ── ClaudeManager ──

    #[tokio::test]
    async fn test_manager_working_dir() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let mgr = ClaudeManager::new(pending);
        assert_eq!(mgr.get_working_dir().await, "");

        mgr.set_working_dir("/tmp/test".to_string()).await;
        assert_eq!(mgr.get_working_dir().await, "/tmp/test");
    }

    #[tokio::test]
    async fn test_manager_working_dir_change() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let mgr = ClaudeManager::new(pending);
        mgr.set_working_dir("/first".to_string()).await;
        mgr.set_working_dir("/second".to_string()).await;
        assert_eq!(mgr.get_working_dir().await, "/second");
    }

    // ── NDJSON multi-line parsing simulation ──

    #[test]
    fn test_parse_ndjson_sequence() {
        let lines = vec![
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hi"}]}}"#,
            r#"{"type":"result","subtype":"success","result":"Hi","is_error":false}"#,
        ];

        let mut events = Vec::new();
        for line in lines {
            let event: ClaudeStreamEvent = serde_json::from_str(line).unwrap();
            events.push(event);
        }

        assert_eq!(events.len(), 3);
        assert!(matches!(&events[0], ClaudeStreamEvent::System { .. }));
        assert!(matches!(&events[1], ClaudeStreamEvent::Assistant { .. }));
        assert!(matches!(&events[2], ClaudeStreamEvent::Result { .. }));
    }

    #[test]
    fn test_parse_unknown_fields_ignored() {
        // Extra fields should be captured in `extra` via #[serde(flatten)]
        let json_str = r#"{
            "type": "system",
            "subtype": "init",
            "session_id": "s1",
            "unknown_field": "should not cause error",
            "model": "claude-sonnet-4-5-20250929"
        }"#;
        let event: ClaudeStreamEvent = serde_json::from_str(json_str).unwrap();
        assert!(matches!(event, ClaudeStreamEvent::System { .. }));
    }
}
