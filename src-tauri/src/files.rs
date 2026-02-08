use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub children: Option<Vec<FileEntry>>,
}

/// List files/directories at a given path (non-recursive, one level)
pub async fn list_directory(path: &str) -> Result<Vec<FileEntry>, String> {
    let dir_path = Path::new(path);
    if !dir_path.is_dir() {
        return Err(format!("フォルダではありません: {}", path));
    }

    let mut entries = Vec::new();
    let mut read_dir = fs::read_dir(dir_path)
        .await
        .map_err(|e| format!("フォルダを読み込めませんでした: {}", e))?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| format!("ファイル情報を読み込めませんでした: {}", e))?
    {
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/directories
        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().await.ok();
        let is_dir = metadata.as_ref().map_or(false, |m| m.is_dir());
        let size = if is_dir {
            None
        } else {
            metadata.as_ref().map(|m| m.len())
        };

        entries.push(FileEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir,
            size,
            children: None,
        });
    }

    // Sort: directories first, then alphabetical
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}

/// Get a shallow tree (depth=1 expansion) of the given path
pub async fn get_file_tree(path: &str) -> Result<FileEntry, String> {
    let dir_path = Path::new(path);
    let name = dir_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());

    let children = list_directory(path).await?;

    Ok(FileEntry {
        name,
        path: path.to_string(),
        is_dir: true,
        size: None,
        children: Some(children),
    })
}

/// Format file size for display
#[allow(dead_code)]
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(500), "500 B");
    }

    #[test]
    fn test_format_size_kb() {
        assert_eq!(format_size(2048), "2.0 KB");
    }

    #[test]
    fn test_format_size_mb() {
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn test_format_size_gb() {
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[tokio::test]
    async fn test_list_directory_exists() {
        // Test with /tmp which should always exist
        let result = list_directory("/tmp").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_directory_not_exists() {
        let result = list_directory("/nonexistent_dir_xyz").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_file_tree() {
        let result = get_file_tree("/tmp").await;
        assert!(result.is_ok());
        let tree = result.unwrap();
        assert!(tree.is_dir);
        assert!(tree.children.is_some());
    }
}
