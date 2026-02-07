mod claude;
mod translator;

use claude::{ChatMessage, ClaudeManager};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

type ClaudeState = Arc<ClaudeManager>;

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    message: String,
) -> Result<(), String> {
    // Emit user message to frontend
    let user_msg = ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: message.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("claude:message", &user_msg);

    state.send_message(&app, message).await
}

#[tauri::command]
async fn set_working_directory(
    state: State<'_, ClaudeState>,
    path: String,
) -> Result<(), String> {
    state.set_working_dir(path).await;
    Ok(())
}

#[tauri::command]
async fn get_working_directory(
    state: State<'_, ClaudeState>,
) -> Result<String, String> {
    Ok(state.get_working_dir().await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let claude_manager = Arc::new(ClaudeManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(claude_manager)
        .invoke_handler(tauri::generate_handler![
            send_message,
            set_working_directory,
            get_working_directory,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
