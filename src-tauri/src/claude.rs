use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::translator::translate_tool_event;

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
}

impl ClaudeManager {
    pub fn new() -> Self {
        Self {
            session_id: Arc::new(Mutex::new(None)),
            working_dir: Mutex::new(String::new()),
        }
    }

    pub async fn get_working_dir(&self) -> String {
        self.working_dir.lock().await.clone()
    }

    pub async fn set_working_dir(&self, dir: String) {
        let mut wd = self.working_dir.lock().await;
        *wd = dir;
    }

    /// Send a user message to Claude Code and stream the response
    pub async fn send_message(
        &self,
        app: &AppHandle,
        message: String,
    ) -> Result<(), String> {
        let working_dir = self.working_dir.lock().await.clone();
        if working_dir.is_empty() {
            return Err("Working directory not set".to_string());
        }

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

        log::info!("Spawning claude with args: {:?}", args);

        // Spawn claude process
        let mut child = Command::new("claude")
            .args(&args)
            .current_dir(&working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn claude: {}", e))?;

        let stdout = child.stdout.take().ok_or("No stdout")?;
        let stderr = child.stderr.take().ok_or("No stderr")?;

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
        let status = child.wait().await.map_err(|e| format!("Process error: {}", e))?;
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
