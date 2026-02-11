use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, oneshot};

use crate::translator::translate_tool_event;

/// Approval request sent to the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub tool_name: String,
    pub description: String,
    pub raw_input: String,
    pub details: Vec<String>,
}

/// Hook payload received from the PreToolUse hook script
#[derive(Debug, Deserialize)]
struct HookPayload {
    tool_name: String,
    tool_input: serde_json::Value,
    #[allow(dead_code)]
    tool_use_id: Option<String>,
    #[allow(dead_code)]
    session_id: Option<String>,
}

/// Response from /respond endpoint
#[derive(Debug, Deserialize)]
struct RespondPayload {
    approval_id: String,
    approved: bool,
}

#[derive(Clone)]
struct ServerState {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    app_handle: AppHandle,
}

/// Tools that are always auto-approved (read-only or safe)
fn is_auto_approved(tool_name: &str, tool_input: &serde_json::Value) -> bool {
    match tool_name {
        // Read-only tools
        "Read" | "Glob" | "Grep" | "WebFetch" | "WebSearch" => true,
        // Task management
        "Task" | "TaskOutput" | "TodoWrite" | "TaskStop" => true,
        // UI-only tools
        "AskUserQuestion" | "EnterPlanMode" | "ExitPlanMode" | "Skill" => true,
        // Team tools
        "TeamCreate" | "TeamDelete" | "SendMessage" => true,
        // MCP read tools
        name if name.starts_with("mcp__") && (
            name.contains("read") || name.contains("list") ||
            name.contains("get") || name.contains("find") ||
            name.contains("search") || name.contains("think") ||
            name.contains("check") || name.contains("initial_instructions") ||
            name.contains("overview")
        ) => true,
        // Bash: check specific commands
        "Bash" => {
            let cmd = tool_input.get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            is_safe_bash_command(cmd)
        }
        _ => false,
    }
}

/// Check if a bash command is safe (read-only)
fn is_safe_bash_command(cmd: &str) -> bool {
    let trimmed = cmd.trim();
    let first_word = trimmed.split_whitespace().next().unwrap_or("");

    matches!(first_word,
        "ls" | "dir" | "pwd" | "echo" | "cat" | "head" | "tail" |
        "whoami" | "hostname" | "date" | "which" | "where" | "type" |
        "find" | "wc" | "sort" | "uniq" | "diff" | "tree"
    ) || trimmed.starts_with("git status")
      || trimmed.starts_with("git log")
      || trimmed.starts_with("git diff")
      || trimmed.starts_with("git branch")
      || trimmed.starts_with("git show")
      || trimmed.starts_with("git remote")
}

/// Build details for the approval dialog
fn build_details(tool_name: &str, tool_input: &serde_json::Value) -> Vec<String> {
    let mut details = Vec::new();

    match tool_name {
        "Bash" => {
            if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
                build_bash_details(cmd, &mut details);
            }
        }
        "Write" => {
            if let Some(path) = tool_input.get("file_path").and_then(|v| v.as_str()) {
                details.push(format!("場所: {}", friendly_path(path)));
            }
        }
        "Edit" => {
            if let Some(path) = tool_input.get("file_path").and_then(|v| v.as_str()) {
                details.push(format!("場所: {}", friendly_path(path)));
            }
            if let Some(old) = tool_input.get("old_string").and_then(|v| v.as_str()) {
                let preview: String = old.chars().take(80).collect();
                details.push(format!("変更箇所: {}...", preview));
            }
        }
        "NotebookEdit" => {
            if let Some(path) = tool_input.get("notebook_path").and_then(|v| v.as_str()) {
                details.push(format!("場所: {}", friendly_path(path)));
            }
        }
        _ => {
            if let Ok(json) = serde_json::to_string_pretty(tool_input) {
                let truncated: String = json.chars().take(300).collect();
                details.push(truncated);
            }
        }
    }

    details
}

/// Parse Bash command into user-friendly detail lines
fn build_bash_details(cmd: &str, details: &mut Vec<String>) {
    let trimmed = cmd.trim();

    if trimmed.starts_with("mkdir") {
        // Show the target directory path
        if let Some(path) = extract_last_path_arg(trimmed) {
            details.push(format!("場所: {}", friendly_path(&path)));
        }
    } else if trimmed.starts_with("cp ") {
        let args = extract_path_args(trimmed);
        if args.len() >= 2 {
            details.push(format!("コピー元: {}", friendly_path(&args[args.len() - 2])));
            details.push(format!("コピー先: {}", friendly_path(args.last().unwrap())));
        }
    } else if trimmed.starts_with("mv ") {
        let args = extract_path_args(trimmed);
        if args.len() >= 2 {
            details.push(format!("移動元: {}", friendly_path(&args[args.len() - 2])));
            details.push(format!("移動先: {}", friendly_path(args.last().unwrap())));
        }
    } else if trimmed.starts_with("rm ") {
        let args = extract_path_args(trimmed);
        for arg in &args {
            details.push(format!("削除対象: {}", friendly_path(arg)));
        }
    } else {
        // Generic: show the full command
        details.push(format!("実行コマンド: {}", cmd));
    }
}

/// Extract path arguments from a command (skipping flags)
fn extract_path_args(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut chars = cmd.chars().peekable();
    let mut in_quote = None;
    let mut current = String::new();
    let mut first = true;

    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                if in_quote == Some(c) {
                    in_quote = None;
                } else if in_quote.is_none() {
                    in_quote = Some(c);
                } else {
                    current.push(c);
                }
            }
            ' ' if in_quote.is_none() => {
                if !current.is_empty() {
                    if first {
                        first = false; // skip command name
                    } else if !current.starts_with('-') {
                        args.push(current.clone());
                    }
                    current.clear();
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() && !first && !current.starts_with('-') {
        args.push(current);
    }
    args
}

/// Extract the last path argument (for mkdir etc.)
fn extract_last_path_arg(cmd: &str) -> Option<String> {
    extract_path_args(cmd).into_iter().last()
}

/// Make a file path more user-friendly (shorten home dir, etc.)
fn friendly_path(path: &str) -> String {
    let cleaned = path.trim_matches(|c| c == '"' || c == '\'');
    if let Some(home) = std::env::var_os("HOME") {
        let home_str = home.to_string_lossy();
        if cleaned.starts_with(home_str.as_ref()) {
            return format!("~{}", &cleaned[home_str.len()..]);
        }
    }
    cleaned.to_string()
}

fn approval_response(approved: bool) -> String {
    serde_json::json!({ "approved": approved }).to_string()
}

async fn handle_approval(
    State(state): State<ServerState>,
    body: String,
) -> Result<String, StatusCode> {
    let payload: HookPayload = serde_json::from_str(&body)
        .map_err(|e| {
            log::error!("Failed to parse hook payload: {} - body: {}", e, &body[..body.len().min(200)]);
            StatusCode::BAD_REQUEST
        })?;

    // Auto-approve safe tools
    if is_auto_approved(&payload.tool_name, &payload.tool_input) {
        return Ok(approval_response(true));
    }

    // Translate tool for human-readable description
    let translated = translate_tool_event(&payload.tool_name, &payload.tool_input);
    let details = build_details(&payload.tool_name, &payload.tool_input);
    let approval_id = uuid::Uuid::new_v4().to_string();

    let approval_request = ApprovalRequest {
        id: approval_id.clone(),
        tool_name: payload.tool_name.clone(),
        description: translated.description,
        raw_input: translated.raw,
        details,
    };

    // Send to frontend
    let _ = state.app_handle.emit("claude:approval_request", &approval_request);

    // Create oneshot channel and wait
    let (tx, rx) = oneshot::channel::<bool>();
    {
        let mut pending = state.pending.lock().await;
        pending.insert(approval_id.clone(), tx);
    }

    log::info!("Waiting for approval: {} ({})", payload.tool_name, approval_id);

    // Wait for response with timeout
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        rx,
    ).await;

    match result {
        Ok(Ok(approved)) => {
            log::info!("Approval response: approved={} for {}", approved, approval_id);
            Ok(approval_response(approved))
        }
        Ok(Err(_)) => {
            log::warn!("Approval channel closed for {}", approval_id);
            Ok(approval_response(true))
        }
        Err(_) => {
            log::warn!("Approval timeout for {}, auto-approving", approval_id);
            state.pending.lock().await.remove(&approval_id);
            Ok(approval_response(true))
        }
    }
}

async fn handle_respond(
    State(state): State<ServerState>,
    body: String,
) -> Result<String, StatusCode> {
    let payload: RespondPayload = serde_json::from_str(&body)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let tx = {
        let mut pending = state.pending.lock().await;
        pending.remove(&payload.approval_id)
    };

    if let Some(tx) = tx {
        let _ = tx.send(payload.approved);
        Ok(format!("{{\"ok\":true,\"approved\":{}}}", payload.approved))
    } else {
        log::warn!("No pending approval found for id: {}", payload.approval_id);
        Err(StatusCode::NOT_FOUND)
    }
}

/// Start the approval HTTP server and return the port
pub async fn start_approval_server(
    app_handle: AppHandle,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
) -> Result<u16, String> {
    let state = ServerState {
        pending,
        app_handle,
    };

    let app = Router::new()
        .route("/approval", post(handle_approval))
        .route("/respond", post(handle_respond))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("承認サーバーを起動できませんでした: {}", e))?;

    let port = listener.local_addr()
        .map_err(|e| format!("ポート取得エラー: {}", e))?
        .port();

    log::info!("Approval server started on port {}", port);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("Approval server error: {}", e);
        }
    });

    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_auto_approve_read_tools() {
        assert!(is_auto_approved("Read", &json!({})));
        assert!(is_auto_approved("Glob", &json!({})));
        assert!(is_auto_approved("Grep", &json!({})));
        assert!(is_auto_approved("WebFetch", &json!({})));
        assert!(is_auto_approved("WebSearch", &json!({})));
    }

    #[test]
    fn test_auto_approve_task_tools() {
        assert!(is_auto_approved("Task", &json!({})));
        assert!(is_auto_approved("TodoWrite", &json!({})));
    }

    #[test]
    fn test_auto_approve_safe_bash() {
        assert!(is_auto_approved("Bash", &json!({"command": "ls -la"})));
        assert!(is_auto_approved("Bash", &json!({"command": "git status"})));
        assert!(is_auto_approved("Bash", &json!({"command": "git log --oneline"})));
        assert!(is_auto_approved("Bash", &json!({"command": "pwd"})));
        assert!(is_auto_approved("Bash", &json!({"command": "echo hello"})));
    }

    #[test]
    fn test_not_auto_approve_dangerous() {
        assert!(!is_auto_approved("Bash", &json!({"command": "rm -rf /"})));
        assert!(!is_auto_approved("Bash", &json!({"command": "npm install foo"})));
        assert!(!is_auto_approved("Bash", &json!({"command": "git push"})));
        assert!(!is_auto_approved("Bash", &json!({"command": "git commit -m \"x\""})));
        assert!(!is_auto_approved("Write", &json!({})));
        assert!(!is_auto_approved("Edit", &json!({})));
    }

    #[test]
    fn test_build_details_bash_rm() {
        let details = build_details("Bash", &json!({"command": "rm -rf /tmp/test"}));
        assert_eq!(details.len(), 1);
        assert!(details[0].contains("削除対象"));
        assert!(details[0].contains("/tmp/test"));
    }

    #[test]
    fn test_build_details_bash_mkdir() {
        let details = build_details("Bash", &json!({"command": "mkdir -p \"/Users/k/Downloads/青色申告_2025\""}));
        assert_eq!(details.len(), 1);
        assert!(details[0].contains("場所"));
        assert!(details[0].contains("青色申告_2025"));
    }

    #[test]
    fn test_build_details_bash_cp() {
        let details = build_details("Bash", &json!({"command": "cp file.txt backup/"}));
        assert_eq!(details.len(), 2);
        assert!(details[0].contains("コピー元"));
        assert!(details[1].contains("コピー先"));
    }

    #[test]
    fn test_build_details_write() {
        let details = build_details("Write", &json!({"file_path": "/home/user/file.txt"}));
        assert_eq!(details.len(), 1);
        assert!(details[0].contains("file.txt"));
    }
}
