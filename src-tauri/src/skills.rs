use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::Mutex;

/// A Claude Code skill (stored as .claude/skills/{name}/SKILL.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoworkSkill {
    pub name: String,        // Directory name = skill name
    pub description: String, // From YAML frontmatter
    pub body: String,        // Markdown body (instructions with $ARGUMENTS)
}

/// Manages Claude Code skills stored as SKILL.md files
pub struct SkillStore {
    working_dir: Mutex<String>,
    legacy_dir: PathBuf,
}

impl SkillStore {
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

    /// Get the skills directory: {working_dir}/.claude/skills/
    async fn skills_dir(&self) -> Result<PathBuf, String> {
        let wd = self.working_dir.lock().await;
        if wd.is_empty() {
            return Err("作業フォルダが設定されていません".to_string());
        }
        Ok(PathBuf::from(wd.as_str()).join(".claude").join("skills"))
    }

    /// Ensure the skills directory exists
    async fn ensure_dir(&self) -> Result<PathBuf, String> {
        let dir = self.skills_dir().await?;
        fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("スキルフォルダを作成できませんでした: {}", e))?;
        Ok(dir)
    }

    /// List all skills from .claude/skills/*/SKILL.md
    pub async fn list(&self) -> Result<Vec<CoworkSkill>, String> {
        let dir = match self.skills_dir().await {
            Ok(d) => d,
            Err(_) => return Ok(Vec::new()),
        };

        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();
        let mut entries = fs::read_dir(&dir)
            .await
            .map_err(|e| format!("スキルフォルダを読み込めませんでした: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("ファイル情報を読み込めませんでした: {}", e))?
        {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_file = path.join("SKILL.md");
            if !skill_file.exists() {
                continue;
            }
            let content = match fs::read_to_string(&skill_file).await {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Failed to read skill file {:?}: {}", skill_file, e);
                    continue;
                }
            };
            match parse_skill_md(&content) {
                Ok(mut skill) => {
                    // Use directory name as skill name if not set in frontmatter
                    if skill.name.is_empty() {
                        if let Some(dir_name) = path.file_name().and_then(|s| s.to_str()) {
                            skill.name = dir_name.to_string();
                        }
                    }
                    skills.push(skill);
                }
                Err(e) => {
                    log::warn!("Failed to parse skill {:?}: {}", skill_file, e);
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Get a single skill by name
    pub async fn get(&self, name: &str) -> Result<CoworkSkill, String> {
        let dir = self.skills_dir().await?;
        let skill_file = dir.join(name).join("SKILL.md");
        if !skill_file.exists() {
            return Err(format!("スキル '{}' が見つかりません", name));
        }
        let content = fs::read_to_string(&skill_file)
            .await
            .map_err(|e| format!("スキルファイルを読み込めませんでした: {}", e))?;
        let mut skill = parse_skill_md(&content)?;
        if skill.name.is_empty() {
            skill.name = name.to_string();
        }
        Ok(skill)
    }

    /// Save a skill as .claude/skills/{name}/SKILL.md
    pub async fn save(&self, skill: &CoworkSkill) -> Result<(), String> {
        let dir = self.ensure_dir().await?;
        let dirname = sanitize_filename(&skill.name);
        let skill_dir = dir.join(&dirname);
        fs::create_dir_all(&skill_dir)
            .await
            .map_err(|e| format!("スキルディレクトリを作成できませんでした: {}", e))?;
        let path = skill_dir.join("SKILL.md");
        let content = serialize_skill_md(skill);
        fs::write(&path, content)
            .await
            .map_err(|e| format!("スキルファイルを書き込めませんでした: {}", e))
    }

    /// Delete a skill by name (removes entire directory)
    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let dir = self.skills_dir().await?;
        let skill_dir = dir.join(name);
        if skill_dir.exists() && skill_dir.is_dir() {
            fs::remove_dir_all(&skill_dir)
                .await
                .map_err(|e| format!("スキルの削除に失敗しました: {}", e))?;
        }
        Ok(())
    }

    /// Migrate legacy JSON skills to .claude/skills/ format
    pub async fn migrate_legacy_skills(&self) -> Result<usize, String> {
        if !self.legacy_dir.exists() {
            return Ok(0);
        }

        let _ = self.skills_dir().await?;

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

                if let Ok(legacy) = serde_json::from_str::<LegacyJsonSkill>(&content) {
                    let skill = CoworkSkill {
                        name: legacy.name.clone(),
                        description: legacy.description,
                        body: convert_template_to_body(&legacy.prompt_template),
                    };

                    if let Err(e) = self.save(&skill).await {
                        log::warn!("Failed to migrate skill '{}': {}", legacy.name, e);
                        continue;
                    }

                    let backup = path.with_extension("json.migrated");
                    let _ = fs::rename(&path, &backup).await;
                    migrated += 1;
                }
            }
        }

        Ok(migrated)
    }

    /// Migrate old .claude/commands/*.md to .claude/skills/ format
    pub async fn migrate_commands_to_skills(&self) -> Result<usize, String> {
        let wd = self.working_dir.lock().await;
        if wd.is_empty() {
            return Ok(0);
        }
        let commands_dir = PathBuf::from(wd.as_str()).join(".claude").join("commands");
        drop(wd);

        if !commands_dir.exists() {
            return Ok(0);
        }

        let mut migrated = 0;
        let mut entries = fs::read_dir(&commands_dir)
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
                    Err(_) => continue,
                };

                match parse_old_command_md(&content) {
                    Ok(mut skill) => {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            skill.name = stem.to_string();
                        }

                        if let Err(e) = self.save(&skill).await {
                            log::warn!("Failed to migrate command '{}': {}", skill.name, e);
                            continue;
                        }

                        let backup = path.with_extension("md.migrated");
                        let _ = fs::rename(&path, &backup).await;
                        migrated += 1;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse command {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(migrated)
    }
}

/// Legacy JSON skill format for migration
#[derive(Debug, Deserialize)]
struct LegacyJsonSkill {
    name: String,
    description: String,
    prompt_template: String,
    #[allow(dead_code)]
    parameters: Vec<LegacyJsonSkillParam>,
}

#[derive(Debug, Deserialize)]
struct LegacyJsonSkillParam {
    #[allow(dead_code)]
    name: String,
}

/// Expand $ARGUMENTS in a skill body
pub fn expand_arguments(body: &str, arguments: &str) -> String {
    body.replace("$ARGUMENTS", arguments)
}

/// Parse a SKILL.md file with optional YAML frontmatter
fn parse_skill_md(content: &str) -> Result<CoworkSkill, String> {
    let trimmed = content.trim();

    if trimmed.starts_with("---") {
        let after_first = &trimmed[3..];
        if let Some(end_idx) = after_first.find("\n---") {
            let frontmatter = after_first[..end_idx].trim();
            let body = after_first[end_idx + 4..].trim();

            let name = extract_field(frontmatter, "name");
            let description = extract_field(frontmatter, "description");

            Ok(CoworkSkill {
                name,
                description,
                body: body.to_string(),
            })
        } else {
            Err("YAML frontmatterの終了マーカー(---)が見つかりません".to_string())
        }
    } else {
        Ok(CoworkSkill {
            name: String::new(),
            description: String::new(),
            body: trimmed.to_string(),
        })
    }
}

/// Parse old .claude/commands/*.md format (description-only frontmatter)
fn parse_old_command_md(content: &str) -> Result<CoworkSkill, String> {
    let trimmed = content.trim();

    if trimmed.starts_with("---") {
        let after_first = &trimmed[3..];
        if let Some(end_idx) = after_first.find("\n---") {
            let frontmatter = after_first[..end_idx].trim();
            let body = after_first[end_idx + 4..].trim();

            let description = extract_field(frontmatter, "description");

            Ok(CoworkSkill {
                name: String::new(),
                description,
                body: body.to_string(),
            })
        } else {
            Err("YAML frontmatterの終了マーカー(---)が見つかりません".to_string())
        }
    } else {
        Ok(CoworkSkill {
            name: String::new(),
            description: String::new(),
            body: trimmed.to_string(),
        })
    }
}

/// Extract a field value from YAML frontmatter (simple key: value parsing)
fn extract_field(frontmatter: &str, key: &str) -> String {
    let prefix = format!("{}:", key);
    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix(&prefix) {
            return rest.trim().trim_matches('"').trim_matches('\'').to_string();
        }
    }
    String::new()
}

/// Serialize a CoworkSkill to SKILL.md format with YAML frontmatter
fn serialize_skill_md(skill: &CoworkSkill) -> String {
    let mut content = String::new();

    let has_name = !skill.name.is_empty();
    let has_desc = !skill.description.is_empty();

    if has_name || has_desc {
        content.push_str("---\n");
        if has_name {
            content.push_str(&format!("name: {}\n", skill.name));
        }
        if has_desc {
            content.push_str(&format!("description: {}\n", skill.description));
        }
        content.push_str("---\n\n");
    }

    content.push_str(&skill.body);
    if !content.ends_with('\n') {
        content.push('\n');
    }

    content
}

/// Replace {{param}} placeholders with $ARGUMENTS for migration
fn convert_template_to_body(template: &str) -> String {
    let re = regex_lite::Regex::new(r"\{\{[^}]+\}\}").unwrap();
    let result = re.replace_all(template, "$$ARGUMENTS");
    let dedup_re = regex_lite::Regex::new(r"(\$ARGUMENTS\s*)+").unwrap();
    dedup_re.replace_all(&result, "$$ARGUMENTS").to_string()
}

/// Sanitize a string for use as a directory/filename
fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c > '\x7f' {
                c
            } else if c == ' ' {
                '-'
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "unnamed-skill".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_md_with_frontmatter() {
        let content = r#"---
name: test-skill
description: テスト用スキル
---

ファイルを整理してください。

$ARGUMENTS
"#;
        let skill = parse_skill_md(content).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.description, "テスト用スキル");
        assert!(skill.body.contains("ファイルを整理してください"));
        assert!(skill.body.contains("$ARGUMENTS"));
    }

    #[test]
    fn test_parse_skill_md_without_frontmatter() {
        let content = "ファイルを整理してください。\n$ARGUMENTS\n";
        let skill = parse_skill_md(content).unwrap();
        assert_eq!(skill.name, "");
        assert_eq!(skill.description, "");
        assert!(skill.body.contains("ファイルを整理してください"));
    }

    #[test]
    fn test_parse_skill_md_invalid_frontmatter() {
        let content = "---\ndescription: テスト\nno closing marker";
        assert!(parse_skill_md(content).is_err());
    }

    #[test]
    fn test_serialize_skill_md_roundtrip() {
        let skill = CoworkSkill {
            name: "test-skill".to_string(),
            description: "テストスキル".to_string(),
            body: "ファイルを$ARGUMENTSで処理して".to_string(),
        };

        let md = serialize_skill_md(&skill);
        let parsed = parse_skill_md(&md).unwrap();

        assert_eq!(parsed.name, skill.name);
        assert_eq!(parsed.description, skill.description);
        assert_eq!(parsed.body, skill.body);
    }

    #[test]
    fn test_serialize_skill_md_no_metadata() {
        let skill = CoworkSkill {
            name: String::new(),
            description: String::new(),
            body: "ファイルを整理して".to_string(),
        };

        let md = serialize_skill_md(&skill);
        assert!(!md.contains("---"));
        assert!(md.contains("ファイルを整理して"));
    }

    #[test]
    fn test_expand_arguments() {
        let body = "以下のファイルを処理してください:\n$ARGUMENTS";
        let result = expand_arguments(body, "C:\\Documents\\file.pdf");
        assert_eq!(
            result,
            "以下のファイルを処理してください:\nC:\\Documents\\file.pdf"
        );
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
        assert_eq!(sanitize_filename(""), "unnamed-skill");
    }

    #[test]
    fn test_extract_field() {
        assert_eq!(
            extract_field("description: \"quoted desc\"", "description"),
            "quoted desc"
        );
        assert_eq!(
            extract_field("description: 'single quoted'", "description"),
            "single quoted"
        );
        assert_eq!(
            extract_field("name: my-skill\ndescription: test", "name"),
            "my-skill"
        );
    }

    #[test]
    fn test_extract_field_missing() {
        assert_eq!(extract_field("other: value", "description"), "");
    }

    #[test]
    fn test_parse_old_command_md() {
        let content = r#"---
description: テスト用コマンド
---

ファイルを整理してください。
"#;
        let skill = parse_old_command_md(content).unwrap();
        assert_eq!(skill.description, "テスト用コマンド");
        assert!(skill.body.contains("ファイルを整理してください"));
    }
}
