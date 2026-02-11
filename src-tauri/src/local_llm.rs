use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, oneshot};

use crate::claude::ChatMessage;

// ── Settings ──

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

// ── OpenAI-compatible API types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ApiFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ApiMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ResponseChoice>,
}

#[derive(Debug, Deserialize)]
struct ResponseChoice {
    message: ResponseMessage,
}

#[derive(Debug, Clone, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ApiToolCall>>,
}

// ── Streaming types (for text-only responses) ──

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: Option<ChunkDelta>,
}

#[derive(Debug, Deserialize)]
struct ChunkDelta {
    content: Option<String>,
    tool_calls: Option<Vec<ChunkToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChunkToolCall {
    index: Option<usize>,
    id: Option<String>,
    function: Option<ChunkFunction>,
}

#[derive(Debug, Deserialize)]
struct ChunkFunction {
    name: Option<String>,
    arguments: Option<String>,
}

// ── Activity item (for frontend activity panel) ──

#[derive(Debug, Clone, Serialize)]
struct ActivityItem {
    id: String,
    description: String,
    raw_command: Option<String>,
    status: String,
    timestamp: String,
}

// ── Approval request (for frontend approval dialog) ──

#[derive(Debug, Clone, Serialize)]
struct ApprovalRequest {
    id: String,
    tool_name: String,
    description: String,
    raw_input: String,
    details: Vec<String>,
}

// ── Tool definitions ──

fn tool_definitions() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read the contents of a file. Returns the file text.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (absolute or relative to working directory)"
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write content to a file. Creates the file if it doesn't exist, overwrites if it does.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path (absolute or relative to working directory)"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file"
                        }
                    },
                    "required": ["path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "list_directory",
                "description": "List files and directories in a path. Returns names with '/' suffix for directories.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Directory path (absolute or relative to working directory)"
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run a shell command and return its output (stdout + stderr).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        }
                    },
                    "required": ["command"]
                }
            }
        }
    ])
}

// ── Tool execution ──

fn resolve_path(path: &str, working_dir: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        PathBuf::from(working_dir).join(p)
    }
}

async fn exec_read_file(path: &str, working_dir: &str) -> String {
    let full = resolve_path(path, working_dir);
    match tokio::fs::read_to_string(&full).await {
        Ok(content) => content,
        Err(e) => format!("Error reading {}: {}", full.display(), e),
    }
}

async fn exec_write_file(path: &str, content: &str, working_dir: &str) -> String {
    let full = resolve_path(path, working_dir);
    // Ensure parent directory exists
    if let Some(parent) = full.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(&full, content).await {
        Ok(()) => format!("Written {} bytes to {}", content.len(), full.display()),
        Err(e) => format!("Error writing {}: {}", full.display(), e),
    }
}

async fn exec_list_directory(path: &str, working_dir: &str) -> String {
    let full = resolve_path(path, working_dir);
    match tokio::fs::read_dir(&full).await {
        Ok(mut entries) => {
            let mut names = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    names.push(format!("{}/", name));
                } else {
                    names.push(name);
                }
            }
            names.sort();
            names.join("\n")
        }
        Err(e) => format!("Error listing {}: {}", full.display(), e),
    }
}

async fn exec_run_command(command: &str, working_dir: &str) -> String {
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let flag = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    match tokio::process::Command::new(shell)
        .arg(flag)
        .arg(command)
        .current_dir(working_dir)
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = String::new();
            if !stdout.is_empty() {
                result.push_str(&stdout);
            }
            if !stderr.is_empty() {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str("[stderr] ");
                result.push_str(&stderr);
            }
            if result.is_empty() {
                format!("Command completed with exit code: {}", output.status.code().unwrap_or(-1))
            } else {
                // Truncate very long output
                if result.len() > 50000 {
                    result.truncate(50000);
                    result.push_str("\n... (truncated)");
                }
                result
            }
        }
        Err(e) => format!("Error running command: {}", e),
    }
}

/// Check if a tool requires user approval
fn requires_approval(tool_name: &str) -> bool {
    matches!(tool_name, "write_file" | "run_command")
}

/// Build a human-readable description for a tool call
fn describe_tool_call(name: &str, args: &serde_json::Value) -> (String, Vec<String>) {
    match name {
        "read_file" => {
            let path = args["path"].as_str().unwrap_or("?");
            (
                format!("ファイルを読み取り: {}", path),
                vec![format!("パス: {}", path)],
            )
        }
        "write_file" => {
            let path = args["path"].as_str().unwrap_or("?");
            let content_len = args["content"].as_str().map(|c| c.len()).unwrap_or(0);
            (
                format!("ファイルに書き込み: {}", path),
                vec![
                    format!("パス: {}", path),
                    format!("サイズ: {} bytes", content_len),
                ],
            )
        }
        "list_directory" => {
            let path = args["path"].as_str().unwrap_or(".");
            (
                format!("ディレクトリ一覧: {}", path),
                vec![format!("パス: {}", path)],
            )
        }
        "run_command" => {
            let cmd = args["command"].as_str().unwrap_or("?");
            (
                format!("コマンド実行: {}", cmd),
                vec![format!("コマンド: {}", cmd)],
            )
        }
        _ => (format!("ツール: {}", name), vec![]),
    }
}

// ── Manager ──

const MAX_TOOL_ROUNDS: usize = 25;

pub struct LocalLlmManager {
    settings: Mutex<LocalLlmSettings>,
    conversation: Mutex<Vec<ApiMessage>>,
    data_dir: Mutex<Option<PathBuf>>,
    working_dir: Mutex<String>,
    approval_pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

impl LocalLlmManager {
    pub fn new(
        approval_pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    ) -> Self {
        Self {
            settings: Mutex::new(LocalLlmSettings::default()),
            conversation: Mutex::new(Vec::new()),
            data_dir: Mutex::new(None),
            working_dir: Mutex::new(String::new()),
            approval_pending,
        }
    }

    pub async fn set_data_dir(&self, dir: PathBuf) {
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

    pub async fn set_working_dir(&self, dir: String) {
        let mut wd = self.working_dir.lock().await;
        *wd = dir;
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

    /// Request approval from the user via the frontend dialog
    async fn request_approval(
        &self,
        app: &AppHandle,
        tool_name: &str,
        description: &str,
        raw_input: &str,
        details: Vec<String>,
    ) -> bool {
        let approval_id = uuid::Uuid::new_v4().to_string();

        let (tx, rx) = oneshot::channel::<bool>();
        {
            let mut pending = self.approval_pending.lock().await;
            pending.insert(approval_id.clone(), tx);
        }

        let req = ApprovalRequest {
            id: approval_id,
            tool_name: tool_name.to_string(),
            description: description.to_string(),
            raw_input: raw_input.to_string(),
            details,
        };
        let _ = app.emit("claude:approval_request", &req);

        // Wait for user response (with timeout)
        match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
            Ok(Ok(approved)) => approved,
            _ => false, // Timeout or channel error → reject
        }
    }

    /// Execute a single tool call
    async fn execute_tool(
        &self,
        app: &AppHandle,
        tool_call: &ApiToolCall,
    ) -> String {
        let working_dir = self.working_dir.lock().await.clone();
        let args: serde_json::Value = match serde_json::from_str(&tool_call.function.arguments) {
            Ok(v) => v,
            Err(e) => return format!("Error parsing arguments: {}", e),
        };

        let (description, details) = describe_tool_call(&tool_call.function.name, &args);

        // Emit activity start
        let activity = ActivityItem {
            id: tool_call.id.clone(),
            description: description.clone(),
            raw_command: Some(tool_call.function.arguments.clone()),
            status: "running".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let _ = app.emit("claude:activity", &activity);

        // Check approval for dangerous tools
        if requires_approval(&tool_call.function.name) {
            let approved = self.request_approval(
                app,
                &tool_call.function.name,
                &description,
                &tool_call.function.arguments,
                details,
            ).await;

            if !approved {
                let done = ActivityItem {
                    id: tool_call.id.clone(),
                    description: "拒否されました".to_string(),
                    raw_command: None,
                    status: "error".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = app.emit("claude:activity_done", &done);
                return "User denied this operation.".to_string();
            }
        }

        // Execute
        let result = match tool_call.function.name.as_str() {
            "read_file" => {
                let path = args["path"].as_str().unwrap_or("");
                exec_read_file(path, &working_dir).await
            }
            "write_file" => {
                let path = args["path"].as_str().unwrap_or("");
                let content = args["content"].as_str().unwrap_or("");
                exec_write_file(path, content, &working_dir).await
            }
            "list_directory" => {
                let path = args["path"].as_str().unwrap_or(".");
                exec_list_directory(path, &working_dir).await
            }
            "run_command" => {
                let command = args["command"].as_str().unwrap_or("");
                exec_run_command(command, &working_dir).await
            }
            other => format!("Unknown tool: {}", other),
        };

        // Emit activity done
        let done = ActivityItem {
            id: tool_call.id.clone(),
            description: "完了".to_string(),
            raw_command: None,
            status: "done".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let _ = app.emit("claude:activity_done", &done);

        result
    }

    /// Build the HTTP client and base request
    fn build_client(&self, settings: &LocalLlmSettings) -> (reqwest::Client, String) {
        let endpoint = settings.endpoint.trim_end_matches('/').to_string();
        let url = format!("{}/chat/completions", endpoint);
        (reqwest::Client::new(), url)
    }

    /// Send a non-streaming request (used during tool-calling loops)
    async fn send_non_streaming(
        &self,
        settings: &LocalLlmSettings,
        messages: &[ApiMessage],
    ) -> Result<ResponseMessage, String> {
        let (client, url) = self.build_client(settings);

        let request = ChatCompletionRequest {
            model: settings.model.clone(),
            messages: messages.to_vec(),
            stream: false,
            tools: Some(tool_definitions()),
        };

        let mut req = client.post(&url)
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(300));

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
            return Err(format!("ローカルLLMエラー ({}): {}", status, body));
        }

        let resp: ChatCompletionResponse = response
            .json()
            .await
            .map_err(|e| format!("レスポンス解析に失敗: {}", e))?;

        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or_else(|| "LLMから応答がありませんでした".to_string())
    }

    /// Send a streaming request (used for final text response without tool calls)
    async fn send_streaming(
        &self,
        app: &AppHandle,
        settings: &LocalLlmSettings,
        messages: &[ApiMessage],
    ) -> Result<(String, Option<Vec<ApiToolCall>>), String> {
        let (client, url) = self.build_client(settings);

        let request = ChatCompletionRequest {
            model: settings.model.clone(),
            messages: messages.to_vec(),
            stream: true,
            tools: Some(tool_definitions()),
        };

        let mut req = client.post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .timeout(std::time::Duration::from_secs(300));

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
            return Err(format!("ローカルLLMエラー ({}): {}", status, body));
        }

        let mut full_text = String::new();
        let mut tool_calls_map: HashMap<usize, (String, String, String)> = HashMap::new(); // index -> (id, name, arguments)
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        use futures_util::StreamExt;
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| format!("ストリーム読み取りエラー: {}", e))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

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
                                // Text content
                                if let Some(ref content) = delta.content {
                                    full_text.push_str(content);
                                    let _ = app.emit("claude:text_delta", content.as_str());
                                }
                                // Tool calls (accumulated across chunks)
                                if let Some(ref tcs) = delta.tool_calls {
                                    for tc in tcs {
                                        let idx = tc.index.unwrap_or(0);
                                        let entry = tool_calls_map
                                            .entry(idx)
                                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                                        if let Some(ref id) = tc.id {
                                            entry.0 = id.clone();
                                        }
                                        if let Some(ref f) = tc.function {
                                            if let Some(ref name) = f.name {
                                                entry.1 = name.clone();
                                            }
                                            if let Some(ref args) = f.arguments {
                                                entry.2.push_str(args);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Process remaining buffer
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
                                full_text.push_str(content);
                                let _ = app.emit("claude:text_delta", content.as_str());
                            }
                            if let Some(ref tcs) = delta.tool_calls {
                                for tc in tcs {
                                    let idx = tc.index.unwrap_or(0);
                                    let entry = tool_calls_map
                                        .entry(idx)
                                        .or_insert_with(|| (String::new(), String::new(), String::new()));
                                    if let Some(ref id) = tc.id {
                                        entry.0 = id.clone();
                                    }
                                    if let Some(ref f) = tc.function {
                                        if let Some(ref name) = f.name {
                                            entry.1 = name.clone();
                                        }
                                        if let Some(ref args) = f.arguments {
                                            entry.2.push_str(args);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Convert accumulated tool calls
        let tool_calls = if tool_calls_map.is_empty() {
            None
        } else {
            let mut calls: Vec<(usize, ApiToolCall)> = tool_calls_map
                .into_iter()
                .map(|(idx, (id, name, args))| {
                    (idx, ApiToolCall {
                        id,
                        call_type: "function".to_string(),
                        function: ApiFunction { name, arguments: args },
                    })
                })
                .collect();
            calls.sort_by_key(|(idx, _)| *idx);
            Some(calls.into_iter().map(|(_, tc)| tc).collect())
        };

        Ok((full_text, tool_calls))
    }

    /// Send a message with full tool-calling loop support
    pub async fn send_message(
        &self,
        app: &AppHandle,
        message: String,
    ) -> Result<(), String> {
        let settings = self.settings.lock().await.clone();

        if !settings.enabled {
            return Err("ローカルLLMが有効化されていません".to_string());
        }

        // Add user message to conversation
        {
            let mut conv = self.conversation.lock().await;
            conv.push(ApiMessage {
                role: "user".to_string(),
                content: Some(message),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Tool-calling loop
        for round in 0..MAX_TOOL_ROUNDS {
            // Build API messages
            let mut api_messages: Vec<ApiMessage> = Vec::new();
            if let Some(ref sys) = settings.system_prompt {
                if !sys.is_empty() {
                    api_messages.push(ApiMessage {
                        role: "system".to_string(),
                        content: Some(sys.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }
            {
                let conv = self.conversation.lock().await;
                api_messages.extend(conv.clone());
            }

            // First round: use streaming for nice UX
            // Subsequent rounds (after tool execution): use non-streaming for simplicity
            let (text, tool_calls) = if round == 0 {
                self.send_streaming(app, &settings, &api_messages).await?
            } else {
                let resp = self.send_non_streaming(&settings, &api_messages).await?;
                // Emit text if any
                if let Some(ref text) = resp.content {
                    if !text.is_empty() {
                        let _ = app.emit("claude:text_delta", text.as_str());
                    }
                }
                (resp.content.unwrap_or_default(), resp.tool_calls)
            };

            let has_tool_calls = tool_calls.as_ref().map(|tc| !tc.is_empty()).unwrap_or(false);

            // Add assistant message to conversation
            {
                let mut conv = self.conversation.lock().await;
                conv.push(ApiMessage {
                    role: "assistant".to_string(),
                    content: if text.is_empty() { None } else { Some(text.clone()) },
                    tool_calls: tool_calls.clone(),
                    tool_call_id: None,
                });
            }

            if !has_tool_calls {
                // No tool calls → final response
                if !text.is_empty() {
                    let msg = ChatMessage {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: "assistant".to_string(),
                        content: text,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    let _ = app.emit("claude:message", &msg);
                }
                break;
            }

            // Emit partial text as a message if any
            if !text.is_empty() {
                let msg = ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "assistant".to_string(),
                    content: text,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = app.emit("claude:message", &msg);
            }

            // Execute tool calls and collect results
            let calls = tool_calls.unwrap();
            for tc in &calls {
                let result = self.execute_tool(app, tc).await;

                // Add tool result to conversation
                {
                    let mut conv = self.conversation.lock().await;
                    conv.push(ApiMessage {
                        role: "tool".to_string(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(tc.id.clone()),
                    });
                }
            }

            // Loop back to send tool results to LLM
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
        assert!(restored.enabled);
        assert_eq!(restored.endpoint, "http://localhost:11434/v1");
        assert_eq!(restored.model, "llama3");
        assert_eq!(restored.api_key.unwrap(), "test-key");
    }

    #[test]
    fn test_parse_sse_chunk_text() {
        let data = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(data).unwrap();
        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(
            chunk.choices[0].delta.as_ref().unwrap().content.as_ref().unwrap(),
            "Hello"
        );
    }

    #[test]
    fn test_parse_sse_chunk_tool_call() {
        let data = r#"{"id":"chatcmpl-1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"/tmp/x\"}"}}]},"finish_reason":null}]}"#;
        let chunk: ChatCompletionChunk = serde_json::from_str(data).unwrap();
        let delta = chunk.choices[0].delta.as_ref().unwrap();
        let tcs = delta.tool_calls.as_ref().unwrap();
        assert_eq!(tcs[0].id.as_ref().unwrap(), "call_1");
        assert_eq!(tcs[0].function.as_ref().unwrap().name.as_ref().unwrap(), "read_file");
    }

    #[test]
    fn test_parse_non_streaming_response() {
        let data = r#"{
            "id": "chatcmpl-1",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\": \"/tmp/x\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(data).unwrap();
        let msg = &resp.choices[0].message;
        assert!(msg.content.is_none());
        let tcs = msg.tool_calls.as_ref().unwrap();
        assert_eq!(tcs[0].function.name, "read_file");
    }

    #[test]
    fn test_parse_non_streaming_text_response() {
        let data = r#"{
            "id": "chatcmpl-2",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "The file contains hello world.",
                    "tool_calls": null
                },
                "finish_reason": "stop"
            }]
        }"#;
        let resp: ChatCompletionResponse = serde_json::from_str(data).unwrap();
        assert_eq!(resp.choices[0].message.content.as_ref().unwrap(), "The file contains hello world.");
        assert!(resp.choices[0].message.tool_calls.is_none());
    }

    #[test]
    fn test_tool_definitions_valid_json() {
        let tools = tool_definitions();
        assert!(tools.is_array());
        assert_eq!(tools.as_array().unwrap().len(), 4);
    }

    #[test]
    fn test_resolve_path_absolute() {
        let result = resolve_path("/home/user/file.txt", "/work");
        assert_eq!(result, PathBuf::from("/home/user/file.txt"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let result = resolve_path("src/main.rs", "/work");
        assert_eq!(result, PathBuf::from("/work/src/main.rs"));
    }

    #[test]
    fn test_requires_approval() {
        assert!(!requires_approval("read_file"));
        assert!(!requires_approval("list_directory"));
        assert!(requires_approval("write_file"));
        assert!(requires_approval("run_command"));
    }

    #[test]
    fn test_describe_tool_call() {
        let args = serde_json::json!({"path": "/tmp/test.txt"});
        let (desc, details) = describe_tool_call("read_file", &args);
        assert!(desc.contains("test.txt"));
        assert!(!details.is_empty());
    }

    #[test]
    fn test_api_message_serialization() {
        // User message
        let msg = ApiMessage {
            role: "user".to_string(),
            content: Some("hello".to_string()),
            tool_calls: None,
            tool_call_id: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"hello\""));
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("tool_call_id"));

        // Tool result message
        let msg = ApiMessage {
            role: "tool".to_string(),
            content: Some("file contents".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"tool_call_id\":\"call_1\""));
    }

    #[tokio::test]
    async fn test_manager_clear_conversation() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let mgr = LocalLlmManager::new(pending);
        {
            let mut conv = mgr.conversation.lock().await;
            conv.push(ApiMessage {
                role: "user".to_string(),
                content: Some("test".to_string()),
                tool_calls: None,
                tool_call_id: None,
            });
        }
        mgr.clear_conversation().await;
        let conv = mgr.conversation.lock().await;
        assert!(conv.is_empty());
    }

    #[tokio::test]
    async fn test_manager_working_dir() {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let mgr = LocalLlmManager::new(pending);
        mgr.set_working_dir("/test/dir".to_string()).await;
        let wd = mgr.working_dir.lock().await.clone();
        assert_eq!(wd, "/test/dir");
    }
}
