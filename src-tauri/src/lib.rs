mod claude;
mod files;
mod skills;
mod todos;
mod translator;

use claude::{ChatMessage, ClaudeManager};
use files::FileEntry;
use skills::{Skill, SkillStore};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use todos::{TodoItem, TodoManager};

type ClaudeState = Arc<ClaudeManager>;
type SkillState = Arc<SkillStore>;
type TodoState = Arc<TodoManager>;

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
async fn list_skills(state: State<'_, SkillState>) -> Result<Vec<Skill>, String> {
    state.list().await
}

#[tauri::command]
async fn save_skill(state: State<'_, SkillState>, skill: Skill) -> Result<(), String> {
    state.save(&skill).await
}

#[tauri::command]
async fn delete_skill(state: State<'_, SkillState>, id: String) -> Result<(), String> {
    state.delete(&id).await
}

#[tauri::command]
async fn execute_skill(
    app: AppHandle,
    claude_state: State<'_, ClaudeState>,
    skill_state: State<'_, SkillState>,
    skill_id: String,
    params: HashMap<String, String>,
) -> Result<(), String> {
    let skill = skill_state
        .get(&skill_id)
        .await?
        .ok_or_else(|| format!("Skill not found: {}", skill_id))?;

    let prompt = skills::expand_template(&skill.prompt_template, &params);

    let user_msg = ChatMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role: "user".to_string(),
        content: format!("⚡ スキル実行: {}\n{}", skill.name, prompt),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let _ = app.emit("claude:message", &user_msg);

    claude_state.send_message(&app, prompt).await
}

// ── TODO commands ──

#[tauri::command]
async fn list_todos(state: State<'_, TodoState>) -> Result<Vec<TodoItem>, String> {
    Ok(state.list().await)
}

#[tauri::command]
async fn add_todo(
    state: State<'_, TodoState>,
    text: String,
    due_date: Option<String>,
) -> Result<TodoItem, String> {
    state.add(text, due_date).await
}

#[tauri::command]
async fn toggle_todo(
    state: State<'_, TodoState>,
    id: String,
) -> Result<Option<TodoItem>, String> {
    state.toggle(&id).await
}

#[tauri::command]
async fn remove_todo(state: State<'_, TodoState>, id: String) -> Result<bool, String> {
    state.remove(&id).await
}

// ── App setup ──

fn get_app_data_dir(app: &tauri::App) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from(".cowork"))
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
            list_files,
            get_file_tree,
            list_skills,
            save_skill,
            delete_skill,
            execute_skill,
            list_todos,
            add_todo,
            toggle_todo,
            remove_todo,
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

            // Initialize skill store
            let skill_store = Arc::new(SkillStore::new(data_dir.clone()));
            app.manage(skill_store);

            // Initialize todo manager
            let todo_manager = Arc::new(TodoManager::new(data_dir));
            let todo_ref = todo_manager.clone();
            tauri::async_runtime::spawn(async move {
                let _ = todo_ref.load().await;
            });
            app.manage(todo_manager);

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
