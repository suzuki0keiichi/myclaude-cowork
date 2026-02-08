use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub done: bool,
    pub created_at: String,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TodoStore {
    items: Vec<TodoItem>,
}

pub struct TodoManager {
    file_path: PathBuf,
    items: Mutex<Vec<TodoItem>>,
}

impl TodoManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let file_path = app_data_dir.join("todos.json");
        Self {
            file_path,
            items: Mutex::new(Vec::new()),
        }
    }

    pub async fn load(&self) -> Result<(), String> {
        if !self.file_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&self.file_path)
            .await
            .map_err(|e| format!("TODOリストを読み込めませんでした: {}", e))?;
        let store: TodoStore = serde_json::from_str(&content)
            .map_err(|e| format!("TODOリストの形式が正しくありません: {}", e))?;
        let mut items = self.items.lock().await;
        *items = store.items;
        Ok(())
    }

    async fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("フォルダを作成できませんでした: {}", e))?;
        }
        let items = self.items.lock().await;
        let store = TodoStore {
            items: items.clone(),
        };
        let content = serde_json::to_string_pretty(&store)
            .map_err(|e| format!("TODOリストの保存に失敗しました: {}", e))?;
        fs::write(&self.file_path, content)
            .await
            .map_err(|e| format!("TODOリストを書き込めませんでした: {}", e))
    }

    pub async fn list(&self) -> Vec<TodoItem> {
        self.items.lock().await.clone()
    }

    pub async fn add(&self, text: String, due_date: Option<String>) -> Result<TodoItem, String> {
        let item = TodoItem {
            id: uuid::Uuid::new_v4().to_string(),
            text,
            done: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            due_date,
        };
        {
            let mut items = self.items.lock().await;
            items.push(item.clone());
        }
        self.save().await?;
        Ok(item)
    }

    pub async fn toggle(&self, id: &str) -> Result<Option<TodoItem>, String> {
        let toggled = {
            let mut items = self.items.lock().await;
            if let Some(item) = items.iter_mut().find(|i| i.id == id) {
                item.done = !item.done;
                Some(item.clone())
            } else {
                None
            }
        };
        if toggled.is_some() {
            self.save().await?;
        }
        Ok(toggled)
    }

    pub async fn remove(&self, id: &str) -> Result<bool, String> {
        let removed = {
            let mut items = self.items.lock().await;
            let len_before = items.len();
            items.retain(|i| i.id != id);
            items.len() < len_before
        };
        if removed {
            self.save().await?;
        }
        Ok(removed)
    }

    #[allow(dead_code)]
    pub async fn update_text(&self, id: &str, text: String) -> Result<Option<TodoItem>, String> {
        let updated = {
            let mut items = self.items.lock().await;
            if let Some(item) = items.iter_mut().find(|i| i.id == id) {
                item.text = text;
                Some(item.clone())
            } else {
                None
            }
        };
        if updated.is_some() {
            self.save().await?;
        }
        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_manager() -> TodoManager {
        let dir = std::env::temp_dir().join(format!("cowork-test-{}", uuid::Uuid::new_v4()));
        TodoManager::new(dir)
    }

    #[tokio::test]
    async fn test_add_and_list() {
        let mgr = temp_manager();
        let item = mgr.add("テストタスク".to_string(), None).await.unwrap();
        assert_eq!(item.text, "テストタスク");
        assert!(!item.done);

        let list = mgr.list().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].text, "テストタスク");
    }

    #[tokio::test]
    async fn test_toggle() {
        let mgr = temp_manager();
        let item = mgr.add("タスク".to_string(), None).await.unwrap();
        assert!(!item.done);

        let toggled = mgr.toggle(&item.id).await.unwrap().unwrap();
        assert!(toggled.done);

        let toggled_back = mgr.toggle(&item.id).await.unwrap().unwrap();
        assert!(!toggled_back.done);
    }

    #[tokio::test]
    async fn test_remove() {
        let mgr = temp_manager();
        let item = mgr.add("削除するタスク".to_string(), None).await.unwrap();

        let removed = mgr.remove(&item.id).await.unwrap();
        assert!(removed);

        let list = mgr.list().await;
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let mgr = temp_manager();
        let removed = mgr.remove("nonexistent-id").await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_update_text() {
        let mgr = temp_manager();
        let item = mgr.add("元のテキスト".to_string(), None).await.unwrap();
        let updated = mgr.update_text(&item.id, "新しいテキスト".to_string()).await.unwrap().unwrap();
        assert_eq!(updated.text, "新しいテキスト");
    }

    #[tokio::test]
    async fn test_persistence() {
        let dir = std::env::temp_dir().join(format!("cowork-test-{}", uuid::Uuid::new_v4()));

        // Create and save
        {
            let mgr = TodoManager::new(dir.clone());
            mgr.add("永続化テスト".to_string(), None).await.unwrap();
        }

        // Load from same path
        {
            let mgr = TodoManager::new(dir.clone());
            mgr.load().await.unwrap();
            let list = mgr.list().await;
            assert_eq!(list.len(), 1);
            assert_eq!(list[0].text, "永続化テスト");
        }

        // Cleanup
        let _ = fs::remove_dir_all(&dir).await;
    }

    #[tokio::test]
    async fn test_multiple_items() {
        let mgr = temp_manager();
        mgr.add("タスク1".to_string(), None).await.unwrap();
        mgr.add("タスク2".to_string(), None).await.unwrap();
        mgr.add("タスク3".to_string(), None).await.unwrap();

        let list = mgr.list().await;
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_with_due_date() {
        let mgr = temp_manager();
        let item = mgr
            .add("期限付き".to_string(), Some("2026-03-01".to_string()))
            .await
            .unwrap();
        assert_eq!(item.due_date, Some("2026-03-01".to_string()));
    }

    #[test]
    fn test_serialization() {
        let item = TodoItem {
            id: "test".to_string(),
            text: "テスト".to_string(),
            done: false,
            created_at: "2026-02-07T00:00:00Z".to_string(),
            due_date: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: TodoItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "テスト");
    }
}
