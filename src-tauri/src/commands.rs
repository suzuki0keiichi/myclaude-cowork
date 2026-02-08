use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::Mutex;

/// A Claude Code command (stored as .claude/commands/{name}.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoworkCommand {
    pub name: String,        // Filename without .md extension = command name
    pub description: String, // From YAML frontmatter
    pub body: String,        // Markdown body (instructions with $ARGUMENTS)
}

/// Manages Claude Code commands stored as .md files
pub struct CommandStore {
    working_dir: Mutex<String>,
    legacy_dir: PathBuf,
}

impl CommandStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            working_dir: Mutex::new(String::new()),
            legacy_dir: app_data_dir.join("skills"),
        }
    }

    pub async fn set_working_dir(&self, dir: String) {
        let mut wd = self.working_dir.lock().await;
        *wd = dir;
    }

    /// Get the commands directory: {working_dir}/.claude/commands/
    async fn commands_dir(&self) -> Result<PathBuf, String> {
        let wd = self.working_dir.lock().await;
        if wd.is_empty() {
            return Err("作業フォルダが設定されていません".to_string());
        }
        Ok(PathBuf::from(wd.as_str()).join(".claude").join("commands"))
    }

    /// Ensure the commands directory exists
    async fn ensure_dir(&self) -> Result<PathBuf, String> {
        let dir = self.commands_dir().await?;
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("コマンドフォルダを作成できませんでした: {}", e))?;
        Ok(dir)
    }

    /// List all commands from .claude/commands/*.md
    pub async fn list(&self) -> Result<Vec<CoworkCommand>, String> {
        let dir = match self.commands_dir().await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut commands = Vec::new();
        let mut entries = fs::read_dir(&dir)
            .await
            .map_err(|e| format!("コマンドフォルダを読み込めませんでした: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("ファイル情報を読み込めませんでした: {}", e))?
        {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "md") {
                let content = match fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(e) => {
                        log::warn!("Failed to read command file {:?}: {}", path, e);
                        continue;
                    }
                };
                match parse_command_md(&content) {
                    Ok(mut cmd) => {
                        // Use filename (without .md) as command name
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            cmd.name = stem.to_string();
                        }
                        commands.push(cmd);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse command {:?}: {}", path, e);
                    }
                }
            }
        }

        commands.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(commands)
    }

    /// Save a command as .claude/commands/{name}.md
    pub async fn save(&self, cmd: &CoworkCommand) -> Result<(), String> {
        let dir = self.ensure_dir().await?;
        let filename = sanitize_filename(&cmd.name);
        let path = dir.join(format!("{}.md", filename));
        let content = serialize_command_md(cmd);
        fs::write(&path, content)
            .await
            .map_err(|e| format!("コマンドファイルを書き込めませんでした: {}", e))
    }

    /// Delete a command by name
    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let dir = self.commands_dir().await?;
        let path = dir.join(format!("{}.md", name));
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| format!("コマンドの削除に失敗しました: {}", e))?;
        }
        Ok(())
    }

    /// Migrate legacy JSON skills to .claude/commands/ format
    pub async fn migrate_legacy_skills(&self) -> Result<usize, String> {
        if !self.legacy_dir.exists() {
            return Ok(0);
        }

        // Check if working dir is set
        let _ = self.commands_dir().await?;

        let mut migrated = 0;
        let mut entries = fs::read_dir(&self.legacy_dir)
            .await
            .map_err(|e| format!("レガシースキルフォルダを読み込めませんでした: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("ファイル情報を読み込めませんでした: {}", e))?
        {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                let content = match fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                // Parse legacy skill JSON
                if let Ok(legacy) = serde_json::from_str::<LegacySkill>(&content) {
                    let cmd = CoworkCommand {
                        name: legacy.name.clone(),
                        description: legacy.description,
                        body: convert_template_to_body(&legacy.prompt_template),
                    };

                    if let Err(e) = self.save(&cmd).await {
                        log::warn!("Failed to migrate skill '{}': {}", legacy.name, e);
                        continue;
                    }

                    // Rename old file to .json.migrated
                    let backup = path.with_extension("json.migrated");
                    let _ = fs::rename(&path, &backup).await;
                    migrated += 1;
                }
            }
        }

        Ok(migrated)
    }
}

/// Legacy skill format for migration
#[derive(Debug, Deserialize)]
struct LegacySkill {
    name: String,
    description: String,
    prompt_template: String,
    #[allow(dead_code)]
    parameters: Vec<LegacySkillParam>,
}

#[derive(Debug, Deserialize)]
struct LegacySkillParam {
    #[allow(dead_code)]
    name: String,
}

/// Parse a .md file with optional YAML frontmatter into a CoworkCommand
fn parse_command_md(content: &str) -> Result<CoworkCommand, String> {
    let trimmed = content.trim();

    if trimmed.starts_with("---") {
        // Has frontmatter
        let after_first = &trimmed[3..];
        if let Some(end_idx) = after_first.find("\n---") {
            let frontmatter = after_first[..end_idx].trim();
            let body = after_first[end_idx + 4..].trim();

            let description = extract_description(frontmatter);

            Ok(CoworkCommand {
                name: String::new(), // Will be set from filename
                description,
                body: body.to_string(),
            })
        } else {
            Err("YAML frontmatterの終了マーカー(---)が見つかりません".to_string())
        }
    } else {
        // No frontmatter, entire content is the body
        Ok(CoworkCommand {
            name: String::new(),
            description: String::new(),
            body: trimmed.to_string(),
        })
    }
}

/// Extract description from YAML frontmatter (simple key: value parsing)
fn extract_description(frontmatter: &str) -> String {
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("description:") {
            return rest.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }
    String::new()
}

/// Serialize a CoworkCommand to .md format with YAML frontmatter
fn serialize_command_md(cmd: &CoworkCommand) -> String {
    let mut content = String::new();

    if !cmd.description.is_empty() {
        content.push_str("---\n");
        content.push_str(&format!("description: {}\n", cmd.description));
        content.push_str("---\n\n");
    }

    content.push_str(&cmd.body);
    if !content.ends_with('\n') {
        content.push('\n');
    }

    content
}

/// Replace {{param}} placeholders with $ARGUMENTS for migration
fn convert_template_to_body(template: &str) -> String {
    // Replace all {{param}} placeholders with $ARGUMENTS
    let re = regex_lite::Regex::new(r"\{\{[^}]+\}\}").unwrap();
    // $$ in replacement string produces literal $
    let result = re.replace_all(template, "$$ARGUMENTS");
    // Deduplicate consecutive $ARGUMENTS
    let dedup_re = regex_lite::Regex::new(r"(\$ARGUMENTS\s*)+").unwrap();
    dedup_re.replace_all(&result, "$$ARGUMENTS").to_string()
}

/// Sanitize a string for use as a filename
fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c > '\x7f' {
                // Allow alphanumeric, hyphens, underscores, and non-ASCII (Japanese)
                c
            } else if c == ' ' {
                '-'
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "unnamed-command".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expand $ARGUMENTS in a command body
    fn expand_arguments(body: &str, arguments: &str) -> String {
        body.replace("$ARGUMENTS", arguments)
    }

    #[test]
    fn test_parse_command_md_with_frontmatter() {
        let content = r#"---
description: テスト用コマンド
---

ファイルを整理してください。

$ARGUMENTS
"#;
        let cmd = parse_command_md(content).unwrap();
        assert_eq!(cmd.description, "テスト用コマンド");
        assert!(cmd.body.contains("ファイルを整理してください"));
        assert!(cmd.body.contains("$ARGUMENTS"));
    }

    #[test]
    fn test_parse_command_md_without_frontmatter() {
        let content = "ファイルを整理してください。\n$ARGUMENTS\n";
        let cmd = parse_command_md(content).unwrap();
        assert_eq!(cmd.description, "");
        assert!(cmd.body.contains("ファイルを整理してください"));
    }

    #[test]
    fn test_parse_command_md_invalid_frontmatter() {
        let content = "---\ndescription: テスト\nno closing marker";
        assert!(parse_command_md(content).is_err());
    }

    #[test]
    fn test_serialize_command_md_roundtrip() {
        let cmd = CoworkCommand {
            name: "test".to_string(),
            description: "テストコマンド".to_string(),
            body: "ファイルを$ARGUMENTSで処理して".to_string(),
        };

        let md = serialize_command_md(&cmd);
        let parsed = parse_command_md(&md).unwrap();

        assert_eq!(parsed.description, cmd.description);
        assert_eq!(parsed.body, cmd.body);
    }

    #[test]
    fn test_serialize_command_md_no_description() {
        let cmd = CoworkCommand {
            name: "test".to_string(),
            description: String::new(),
            body: "ファイルを整理して".to_string(),
        };

        let md = serialize_command_md(&cmd);
        assert!(!md.contains("---"));
        assert!(md.contains("ファイルを整理して"));
    }

    #[test]
    fn test_expand_arguments() {
        let body = "以下のファイルを処理してください:\n$ARGUMENTS";
        let result = expand_arguments(body, "C:\\Documents\\file.pdf");
        assert_eq!(result, "以下のファイルを処理してください:\nC:\\Documents\\file.pdf");
    }

    #[test]
    fn test_expand_arguments_no_placeholder() {
        let body = "全てのPDFを整理して";
        let result = expand_arguments(body, "some context");
        assert_eq!(result, "全てのPDFを整理して");
    }

    #[test]
    fn test_convert_template_to_body() {
        let template = "{{folder}}の中のPDFを{{action}}してください";
        let body = convert_template_to_body(template);
        assert_eq!(body, "$ARGUMENTSの中のPDFを$ARGUMENTSしてください");
    }

    #[test]
    fn test_convert_template_no_params() {
        let template = "ファイル一覧を表示して";
        let body = convert_template_to_body(template);
        assert_eq!(body, "ファイル一覧を表示して");
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("請求書振り分け"), "請求書振り分け");
        assert_eq!(sanitize_filename("test command"), "test-command");
        assert_eq!(sanitize_filename("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_filename(""), "unnamed-command");
    }

    #[test]
    fn test_extract_description_quoted() {
        assert_eq!(
            extract_description("description: \"quoted desc\""),
            "quoted desc"
        );
        assert_eq!(
            extract_description("description: 'single quoted'"),
            "single quoted"
        );
    }

    #[test]
    fn test_extract_description_missing() {
        assert_eq!(extract_description("other: value"), "");
    }
}
