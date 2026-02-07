use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// A saved skill (reusable workflow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub prompt_template: String,     // The prompt to send to Claude, with {{placeholders}}
    pub parameters: Vec<SkillParam>, // User-fillable parameters
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParam {
    pub name: String,
    pub label: String,        // Japanese display label
    pub param_type: String,   // "text" | "path" | "select"
    pub default_value: String,
    pub options: Vec<String>, // For "select" type
}

/// Manages skill persistence
pub struct SkillStore {
    dir: PathBuf,
}

impl SkillStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let dir = app_data_dir.join("skills");
        Self { dir }
    }

    async fn ensure_dir(&self) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .await
            .map_err(|e| format!("Failed to create skills dir: {}", e))
    }

    pub async fn list(&self) -> Result<Vec<Skill>, String> {
        self.ensure_dir().await?;
        let mut skills = Vec::new();

        let mut entries = fs::read_dir(&self.dir)
            .await
            .map_err(|e| format!("Failed to read skills dir: {}", e))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read entry: {}", e))?
        {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                let content = fs::read_to_string(&path)
                    .await
                    .map_err(|e| format!("Failed to read skill file: {}", e))?;
                if let Ok(skill) = serde_json::from_str::<Skill>(&content) {
                    skills.push(skill);
                }
            }
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    pub async fn save(&self, skill: &Skill) -> Result<(), String> {
        self.ensure_dir().await?;
        let path = self.dir.join(format!("{}.json", skill.id));
        let content = serde_json::to_string_pretty(skill)
            .map_err(|e| format!("Failed to serialize skill: {}", e))?;
        fs::write(&path, content)
            .await
            .map_err(|e| format!("Failed to write skill: {}", e))
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| format!("Failed to delete skill: {}", e))?;
        }
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<Skill>, String> {
        let path = self.dir.join(format!("{}.json", id));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| format!("Failed to read skill: {}", e))?;
        let skill = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse skill: {}", e))?;
        Ok(Some(skill))
    }
}

/// Expand a skill's prompt template with given parameter values
pub fn expand_template(template: &str, params: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_expand_template_basic() {
        let template = "{{folder}}の中のPDFを{{action}}してください";
        let mut params = HashMap::new();
        params.insert("folder".to_string(), "請求書フォルダ".to_string());
        params.insert("action".to_string(), "振り分け".to_string());
        let result = expand_template(template, &params);
        assert_eq!(result, "請求書フォルダの中のPDFを振り分けしてください");
    }

    #[test]
    fn test_expand_template_no_params() {
        let template = "ファイル一覧を表示して";
        let params = HashMap::new();
        let result = expand_template(template, &params);
        assert_eq!(result, "ファイル一覧を表示して");
    }

    #[test]
    fn test_expand_template_missing_param() {
        let template = "{{folder}}を整理して";
        let params = HashMap::new();
        let result = expand_template(template, &params);
        // Missing param stays as-is
        assert_eq!(result, "{{folder}}を整理して");
    }

    #[test]
    fn test_skill_serialization() {
        let skill = Skill {
            id: "test-1".to_string(),
            name: "請求書振り分け".to_string(),
            description: "PDFの請求書を取引先ごとに振り分けます".to_string(),
            prompt_template: "{{folder}}の請求書を振り分けて".to_string(),
            parameters: vec![SkillParam {
                name: "folder".to_string(),
                label: "対象フォルダ".to_string(),
                param_type: "path".to_string(),
                default_value: "".to_string(),
                options: vec![],
            }],
            created_at: "2026-02-07T00:00:00Z".to_string(),
            updated_at: "2026-02-07T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&skill).unwrap();
        let parsed: Skill = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "請求書振り分け");
        assert_eq!(parsed.parameters.len(), 1);
        assert_eq!(parsed.parameters[0].label, "対象フォルダ");
    }
}
