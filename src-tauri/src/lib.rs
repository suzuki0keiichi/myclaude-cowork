mod approval_server;
mod claude;
mod files;
mod gdrive;
mod local_llm;
mod oauth_server;
mod skills;
mod slack;
mod todos;
mod translator;

use claude::{ChatMessage, ClaudeManager};
use files::FileEntry;
use gdrive::{DriveFile, GDriveClient};
use local_llm::{LocalLlmManager, LocalLlmSettings};
use skills::{CoworkSkill, SkillStore};
use slack::{SlackClient, SlackListItem, SlackSettings};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use todos::{TodoItem, TodoManager};
use tokio::sync::{Mutex, oneshot};

type ClaudeState = Arc<ClaudeManager>;
type SkillState = Arc<SkillStore>;
type GDriveState = Arc<GDriveClient>;
type SlackState = Arc<SlackClient>;
type TodoState = Arc<TodoManager>;
type LocalLlmState = Arc<LocalLlmManager>;
type ApprovalPendingState = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

// ── Claude commands ──

#[tauri::command]
async fn send_message(
    app: AppHandle,
    state: State<'_, ClaudeState>,
    message: String,
) -> Result<(), String> {
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
    app: AppHandle,
    state: State<'_, ClaudeState>,
    skill_state: State<'_, SkillState>,
    llm_state: State<'_, LocalLlmState>,
    path: String,
) -> Result<(), String> {
    state.set_working_dir(path.clone()).await;
    skill_state.set_working_dir(path.clone()).await;
    llm_state.set_working_dir(path.clone()).await;

    if !path.is_empty() {
        if let Err(e) = save_last_working_dir(&app, &path).await {
            log::warn!("Failed to save working dir: {}", e);
        }

        // Migrate legacy JSON skills
        match skill_state.migrate_legacy_skills().await {
            Ok(count) if count > 0 => {
                log::info!("Migrated {} legacy JSON skills", count);
                let msg = ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "system".to_string(),
                    content: format!("{}件のレガシースキルを移行しました", count),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = app.emit("claude:message", &msg);
            }
            Err(e) => {
                log::warn!("Legacy skill migration failed: {}", e);
            }
            _ => {}
        }

        // Migrate old .claude/commands/ to .claude/skills/
        match skill_state.migrate_commands_to_skills().await {
            Ok(count) if count > 0 => {
                log::info!("Migrated {} commands to skills format", count);
                let msg = ChatMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    role: "system".to_string(),
                    content: format!("{}件のコマンドをスキル形式に移行しました", count),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = app.emit("claude:message", &msg);
            }
            Err(e) => {
                log::warn!("Command to skill migration failed: {}", e);
            }
            _ => {}
        }
    }

    Ok(())
}

#[tauri::command]
async fn cancel_message(state: State<'_, ClaudeState>) -> Result<(), String> {
    state.cancel().await
}

#[tauri::command]
async fn get_working_directory(state: State<'_, ClaudeState>) -> Result<String, String> {
    Ok(state.get_working_dir().await)
}

#[tauri::command]
async fn respond_to_approval(
    state: State<'_, ApprovalPendingState>,
    approval_id: String,
    approved: bool,
) -> Result<(), String> {
    let tx = {
        let mut pending = state.lock().await;
        pending.remove(&approval_id)
    };

    if let Some(tx) = tx {
        let _ = tx.send(approved);
        Ok(())
    } else {
        Err(format!("承認リクエストが見つかりません: {}", approval_id))
    }
}

// ── File browser commands ──

#[tauri::command]
async fn list_files(path: String) -> Result<Vec<FileEntry>, String> {
    files::list_directory(&path).await
}

#[tauri::command]
async fn get_file_tree(path: String) -> Result<FileEntry, String> {
    files::get_file_tree(&path).await
}

// ── Skill commands ──

#[tauri::command]
async fn list_skills(state: State<'_, SkillState>) -> Result<Vec<CoworkSkill>, String> {
    state.list().await
}

#[tauri::command]
async fn save_skill(
    state: State<'_, SkillState>,
    skill: CoworkSkill,
) -> Result<(), String> {
    state.save(&skill).await
}

#[tauri::command]
async fn delete_skill(state: State<'_, SkillState>, name: String) -> Result<(), String> {
    state.delete(&name).await
}

#[tauri::command]
async fn execute_skill(
    app: AppHandle,
    claude_state: State<'_, ClaudeState>,
    name: String,
    context: String,
) -> Result<(), String> {
    // Send /{skill-name} to Claude Code CLI — it handles skill expansion natively
    let message = if context.is_empty() {
        format!("/{}", name)
    } else {
        format!("/{} {}", name, context)
    };

    let user_msg = ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: message.clone(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("claude:message", &user_msg);

    claude_state.send_message(&app, message).await
}

// ── TODO commands ──

#[tauri::command]
async fn todo_list(state: State<'_, TodoState>) -> Result<Vec<TodoItem>, String> {
    Ok(state.list().await)
}

#[tauri::command]
async fn todo_add(
    state: State<'_, TodoState>,
    text: String,
    due_date: Option<String>,
) -> Result<TodoItem, String> {
    state.add(text, due_date).await
}

#[tauri::command]
async fn todo_toggle(
    state: State<'_, TodoState>,
    id: String,
) -> Result<Option<TodoItem>, String> {
    state.toggle(&id).await
}

#[tauri::command]
async fn todo_remove(state: State<'_, TodoState>, id: String) -> Result<bool, String> {
    state.remove(&id).await
}

// ── Google Drive commands ──

#[tauri::command]
async fn gdrive_is_configured(state: State<'_, GDriveState>) -> Result<bool, String> {
    Ok(state.is_configured().await)
}

#[tauri::command]
async fn gdrive_is_authenticated(state: State<'_, GDriveState>) -> Result<bool, String> {
    Ok(state.is_authenticated().await)
}

/// Start the OAuth flow: returns the auth URL for the frontend to open.
/// A background task waits for the callback and emits events on completion.
#[tauri::command]
async fn gdrive_start_auth(
    app: AppHandle,
    state: State<'_, GDriveState>,
) -> Result<String, String> {
    let (url, port, rx) = state.start_auth_flow().await?;

    let gdrive = state.inner().clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        let result = tokio::time::timeout(std::time::Duration::from_secs(300), rx).await;
        match result {
            Ok(Ok(Ok(code))) => match gdrive.exchange_code(&code, port).await {
                Ok(()) => {
                    let _ = app_clone.emit("gdrive:auth_complete", ());
                }
                Err(e) => {
                    let _ = app_clone.emit("gdrive:auth_error", &e);
                }
            },
            Ok(Ok(Err(e))) => {
                let _ = app_clone.emit("gdrive:auth_error", &e);
            }
            Ok(Err(_)) => {
                let _ = app_clone.emit("gdrive:auth_error", "認証がキャンセルされました");
            }
            Err(_) => {
                let _ = app_clone.emit("gdrive:auth_error", "認証がタイムアウトしました（5分）");
            }
        }
    });

    Ok(url)
}

#[tauri::command]
async fn gdrive_logout(state: State<'_, GDriveState>) -> Result<(), String> {
    state.logout().await
}

#[tauri::command]
async fn gdrive_list_files(
    state: State<'_, GDriveState>,
    folder_id: Option<String>,
) -> Result<Vec<DriveFile>, String> {
    state.list_files(folder_id.as_deref()).await
}

#[tauri::command]
async fn gdrive_download_file(
    state: State<'_, GDriveState>,
    file_id: String,
    dest: String,
) -> Result<String, String> {
    state.download_file(&file_id, &dest).await
}

// ── Slack commands ──

#[tauri::command]
async fn slack_is_configured(state: State<'_, SlackState>) -> Result<bool, String> {
    Ok(state.is_configured().await)
}

#[tauri::command]
async fn slack_is_authenticated(state: State<'_, SlackState>) -> Result<bool, String> {
    Ok(state.is_authenticated().await)
}

#[tauri::command]
async fn slack_get_team_name(state: State<'_, SlackState>) -> Result<Option<String>, String> {
    Ok(state.get_team_name().await)
}

#[tauri::command]
async fn slack_get_settings(state: State<'_, SlackState>) -> Result<SlackSettings, String> {
    Ok(state.get_settings().await)
}

#[tauri::command]
async fn slack_save_settings(
    state: State<'_, SlackState>,
    settings: SlackSettings,
) -> Result<(), String> {
    state.save_settings(settings).await
}

/// Start Slack OAuth flow. Returns the auth URL.
#[tauri::command]
async fn slack_start_auth(
    app: AppHandle,
    state: State<'_, SlackState>,
) -> Result<String, String> {
    let (url, port, rx) = state.start_auth_flow().await?;

    let slack = state.inner().clone();
    let app_clone = app.clone();
    tokio::spawn(async move {
        let result = tokio::time::timeout(std::time::Duration::from_secs(300), rx).await;
        match result {
            Ok(Ok(Ok(code))) => match slack.exchange_code(&code, port).await {
                Ok(()) => {
                    let _ = app_clone.emit("slack:auth_complete", ());
                }
                Err(e) => {
                    let _ = app_clone.emit("slack:auth_error", &e);
                }
            },
            Ok(Ok(Err(e))) => {
                let _ = app_clone.emit("slack:auth_error", &e);
            }
            Ok(Err(_)) => {
                let _ = app_clone.emit("slack:auth_error", "認証がキャンセルされました");
            }
            Err(_) => {
                let _ = app_clone.emit("slack:auth_error", "認証がタイムアウトしました（5分）");
            }
        }
    });

    Ok(url)
}

#[tauri::command]
async fn slack_logout(state: State<'_, SlackState>) -> Result<(), String> {
    state.logout().await
}

#[tauri::command]
async fn slack_list_items(
    state: State<'_, SlackState>,
    list_id: String,
) -> Result<Vec<SlackListItem>, String> {
    state.list_items(&list_id).await
}

#[tauri::command]
async fn slack_create_item(
    state: State<'_, SlackState>,
    list_id: String,
    title: String,
) -> Result<SlackListItem, String> {
    state.create_item(&list_id, &title).await
}

// ── Local LLM commands ──

#[tauri::command]
async fn local_llm_get_settings(
    state: State<'_, LocalLlmState>,
) -> Result<LocalLlmSettings, String> {
    Ok(state.get_settings().await)
}

#[tauri::command]
async fn local_llm_save_settings(
    state: State<'_, LocalLlmState>,
    settings: LocalLlmSettings,
) -> Result<(), String> {
    state.save_settings(settings).await
}

#[tauri::command]
async fn local_llm_send_message(
    app: AppHandle,
    state: State<'_, LocalLlmState>,
    message: String,
) -> Result<(), String> {
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
async fn local_llm_test_connection(
    state: State<'_, LocalLlmState>,
) -> Result<String, String> {
    state.test_connection().await
}

#[tauri::command]
async fn local_llm_clear_conversation(
    state: State<'_, LocalLlmState>,
) -> Result<(), String> {
    state.clear_conversation().await;
    Ok(())
}

// ── Working directory persistence ──

#[tauri::command]
async fn get_last_working_dir(app: AppHandle) -> Result<String, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let path = data_dir.join("last_working_dir.txt");
    if !path.exists() {
        return Ok(String::new());
    }
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("前回の作業フォルダの読み込みに失敗: {}", e))
}

async fn save_last_working_dir(app: &AppHandle, dir: &str) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tokio::fs::create_dir_all(&data_dir)
        .await
        .map_err(|e| format!("ディレクトリ作成に失敗: {}", e))?;
    let path = data_dir.join("last_working_dir.txt");
    tokio::fs::write(&path, dir)
        .await
        .map_err(|e| format!("作業フォルダの保存に失敗: {}", e))
}

// ── Chat history commands ──

#[tauri::command]
async fn chat_load_messages(app: AppHandle) -> Result<Vec<ChatMessage>, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let path = data_dir.join("chat_messages.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("チャット履歴の読み込みに失敗: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("チャット履歴の解析に失敗: {}", e))
}

#[tauri::command]
async fn chat_save_messages(app: AppHandle, messages: Vec<ChatMessage>) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    tokio::fs::create_dir_all(&data_dir)
        .await
        .map_err(|e| format!("ディレクトリ作成に失敗: {}", e))?;
    let path = data_dir.join("chat_messages.json");
    let content = serde_json::to_string(&messages)
        .map_err(|e| format!("チャット履歴のシリアライズに失敗: {}", e))?;
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| format!("チャット履歴の保存に失敗: {}", e))
}

#[tauri::command]
async fn chat_clear_messages(app: AppHandle) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let path = data_dir.join("chat_messages.json");
    if path.exists() {
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| format!("チャット履歴の削除に失敗: {}", e))?;
    }
    Ok(())
}

#[tauri::command]
async fn reset_session(state: State<'_, ClaudeState>) -> Result<(), String> {
    state.reset_session().await;
    Ok(())
}

// ── App setup ──

fn get_app_data_dir(app: &tauri::App) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from(".cowork"))
}

fn get_resource_dir(app: &tauri::App) -> Option<PathBuf> {
    app.path().resource_dir().ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let approval_pending: ApprovalPendingState = Arc::new(Mutex::new(HashMap::new()));
    let claude_manager = Arc::new(ClaudeManager::new(Arc::clone(&approval_pending)));
    let local_llm_manager = Arc::new(LocalLlmManager::new(Arc::clone(&approval_pending)));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(claude_manager)
        .manage(local_llm_manager)
        .manage(approval_pending)
        .invoke_handler(tauri::generate_handler![
            send_message,
            cancel_message,
            set_working_directory,
            get_working_directory,
            list_files,
            get_file_tree,
            list_skills,
            save_skill,
            delete_skill,
            execute_skill,
            todo_list,
            todo_add,
            todo_toggle,
            todo_remove,
            // Google Drive
            gdrive_is_configured,
            gdrive_is_authenticated,
            gdrive_start_auth,
            gdrive_logout,
            gdrive_list_files,
            gdrive_download_file,
            // Slack
            slack_is_configured,
            slack_is_authenticated,
            slack_get_team_name,
            slack_get_settings,
            slack_save_settings,
            slack_start_auth,
            slack_logout,
            slack_list_items,
            slack_create_item,
            // Local LLM
            local_llm_get_settings,
            local_llm_save_settings,
            local_llm_send_message,
            local_llm_test_connection,
            local_llm_clear_conversation,
            // Other
            respond_to_approval,
            get_last_working_dir,
            chat_load_messages,
            chat_save_messages,
            chat_clear_messages,
            reset_session,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            let data_dir = get_app_data_dir(app);
            let resource_dir = get_resource_dir(app);

            // Install hook for Claude Code approval flow
            if let Err(e) = ClaudeManager::ensure_hook_installed(app.handle()) {
                log::warn!("Hook installation failed: {}", e);
            }

            // Restore saved session ID so conversations persist across restarts
            let claude: Arc<ClaudeManager> = app.state::<ClaudeState>().inner().clone();
            let data_dir_for_session = data_dir.clone();
            tauri::async_runtime::block_on(claude.set_data_dir(data_dir_for_session));

            // Initialize skill store
            let skill_store = Arc::new(SkillStore::new(data_dir.clone()));
            app.manage(skill_store);

            // Initialize todo manager
            let todo_manager = Arc::new(TodoManager::new(data_dir.clone()));
            let todo_ref = todo_manager.clone();
            tauri::async_runtime::spawn(async move {
                let _ = todo_ref.load().await;
            });
            app.manage(todo_manager);

            // Initialize local LLM manager
            let local_llm: Arc<LocalLlmManager> = app.state::<LocalLlmState>().inner().clone();
            let data_dir_for_llm = data_dir.clone();
            tauri::async_runtime::spawn(async move {
                local_llm.set_data_dir(data_dir_for_llm).await;
            });

            // Initialize Google Drive client
            let gdrive_client =
                Arc::new(GDriveClient::new(data_dir.clone(), resource_dir.clone()));
            let gdrive_ref = gdrive_client.clone();
            tauri::async_runtime::spawn(async move {
                let _ = gdrive_ref.load().await;
            });
            app.manage(gdrive_client);

            // Initialize Slack client
            let slack_client = Arc::new(SlackClient::new(data_dir, resource_dir));
            let slack_ref = slack_client.clone();
            tauri::async_runtime::spawn(async move {
                let _ = slack_ref.load().await;
            });
            app.manage(slack_client);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
