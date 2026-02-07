use serde_json::Value;

pub struct TranslatedTool {
    pub description: String,
    pub raw: String,
}

/// Translate a Claude Code tool invocation into human-readable Japanese
pub fn translate_tool_event(tool_name: &str, input: &Value) -> TranslatedTool {
    let raw = format!("{}({})", tool_name, serde_json::to_string(input).unwrap_or_default());

    let description = match tool_name {
        "Bash" => translate_bash(input),
        "Read" => translate_read(input),
        "Write" => translate_write(input),
        "Edit" => translate_edit(input),
        "Glob" => translate_glob(input),
        "Grep" => translate_grep(input),
        "TodoWrite" => "TODOリストを更新しています".to_string(),
        "WebFetch" => translate_web_fetch(input),
        "WebSearch" => translate_web_search(input),
        "Task" => translate_task(input),
        "NotebookEdit" => translate_notebook(input),
        _ => format!("ツール「{}」を実行中", tool_name),
    };

    TranslatedTool { description, raw }
}

fn translate_bash(input: &Value) -> String {
    let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");

    // File operations
    if cmd.starts_with("mv ") || cmd.contains(" mv ") {
        return extract_file_op(cmd, "ファイルを移動します");
    }
    if cmd.starts_with("cp ") || cmd.contains(" cp ") {
        return extract_file_op(cmd, "ファイルをコピーします");
    }
    if cmd.starts_with("mkdir ") || cmd.contains(" mkdir ") {
        return extract_mkdir(cmd);
    }
    if cmd.starts_with("rm ") || cmd.contains(" rm ") {
        return format!("⚠️ ファイルを削除します: {}", summarize_path(cmd));
    }

    // Git
    if cmd.starts_with("git ") {
        return translate_git(cmd);
    }

    // Network
    if cmd.starts_with("curl ") || cmd.starts_with("wget ") || cmd.contains("fetch") {
        return "外部サービスに接続しています".to_string();
    }

    // npm/node
    if cmd.starts_with("npm ") || cmd.starts_with("npx ") || cmd.starts_with("node ") {
        return format!("コマンドを実行しています: {}", truncate(cmd, 60));
    }

    // Python
    if cmd.starts_with("python") || cmd.starts_with("pip") {
        return format!("Pythonコマンドを実行しています: {}", truncate(cmd, 60));
    }

    // ls / listing
    if cmd.starts_with("ls ") || cmd == "ls" {
        return "フォルダの中身を確認しています".to_string();
    }

    // Generic
    format!("コマンドを実行しています: {}", truncate(cmd, 60))
}

fn translate_read(input: &Value) -> String {
    let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("ファイル");
    let name = extract_filename(path);
    format!("📄 「{}」を読んでいます", name)
}

fn translate_write(input: &Value) -> String {
    let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("ファイル");
    let name = extract_filename(path);
    format!("📝 「{}」を作成しています", name)
}

fn translate_edit(input: &Value) -> String {
    let path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("ファイル");
    let name = extract_filename(path);
    format!("✏️ 「{}」を編集しています", name)
}

fn translate_glob(input: &Value) -> String {
    let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("*");
    format!("🔍 ファイルを検索しています: {}", pattern)
}

fn translate_grep(input: &Value) -> String {
    let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
    format!("🔍 ファイル内を検索しています: 「{}」", truncate(pattern, 40))
}

fn translate_web_fetch(input: &Value) -> String {
    let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("URL");
    format!("🌐 Webページを取得しています: {}", truncate(url, 50))
}

fn translate_web_search(input: &Value) -> String {
    let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
    format!("🔍 Web検索しています: 「{}」", truncate(query, 40))
}

fn translate_task(input: &Value) -> String {
    let desc = input.get("description").and_then(|v| v.as_str()).unwrap_or("タスク");
    format!("⚙️ サブタスクを実行中: {}", truncate(desc, 50))
}

fn translate_notebook(input: &Value) -> String {
    let path = input.get("notebook_path").and_then(|v| v.as_str()).unwrap_or("ノートブック");
    let name = extract_filename(path);
    format!("📓 ノートブック「{}」を編集しています", name)
}

fn translate_git(cmd: &str) -> String {
    if cmd.contains("status") {
        return "Gitの状態を確認しています".to_string();
    }
    if cmd.contains("diff") {
        return "変更内容を確認しています".to_string();
    }
    if cmd.contains("log") {
        return "コミット履歴を確認しています".to_string();
    }
    if cmd.contains("add") {
        return "変更をステージングしています".to_string();
    }
    if cmd.contains("commit") {
        return "変更を保存（コミット）しています".to_string();
    }
    if cmd.contains("push") {
        return "変更をリモートに送信しています".to_string();
    }
    if cmd.contains("pull") || cmd.contains("fetch") {
        return "最新の変更を取得しています".to_string();
    }
    if cmd.contains("checkout") || cmd.contains("switch") {
        return "ブランチを切り替えています".to_string();
    }
    format!("Git操作を実行しています: {}", truncate(cmd, 50))
}

fn extract_file_op(cmd: &str, op_desc: &str) -> String {
    // Try to extract source and dest from the command
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.len() >= 3 {
        let src = extract_filename(parts[parts.len() - 2]);
        let dst = extract_filename(parts[parts.len() - 1]);
        format!("{}: 「{}」→「{}」", op_desc, src, dst)
    } else {
        op_desc.to_string()
    }
}

fn extract_mkdir(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if let Some(dir) = parts.last() {
        let name = extract_filename(dir);
        format!("📁 フォルダ「{}」を作成しています", name)
    } else {
        "📁 フォルダを作成しています".to_string()
    }
}

fn extract_filename(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn summarize_path(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    parts.iter()
        .filter(|p| !p.starts_with('-'))
        .skip(1) // skip command name
        .map(|p| extract_filename(p))
        .collect::<Vec<_>>()
        .join(", ")
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── translate_tool_event dispatch ──

    #[test]
    fn test_bash_mv() {
        let input = json!({"command": "mv /a/b.txt /c/d/"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("移動"));
        assert!(t.description.contains("b.txt"));
        assert!(t.raw.starts_with("Bash("));
    }

    #[test]
    fn test_bash_cp() {
        let input = json!({"command": "cp file.txt backup/"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("コピー"));
    }

    #[test]
    fn test_bash_mkdir() {
        let input = json!({"command": "mkdir -p /home/user/新しいフォルダ"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("フォルダ"));
        assert!(t.description.contains("新しいフォルダ"));
    }

    #[test]
    fn test_bash_rm_warning() {
        let input = json!({"command": "rm -rf /tmp/junk"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("⚠️"));
        assert!(t.description.contains("削除"));
    }

    #[test]
    fn test_bash_git_status() {
        let input = json!({"command": "git status"});
        let t = translate_tool_event("Bash", &input);
        assert_eq!(t.description, "Gitの状態を確認しています");
    }

    #[test]
    fn test_bash_git_commit() {
        let input = json!({"command": "git commit -m \"fix\""});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("コミット"));
    }

    #[test]
    fn test_bash_git_push() {
        let input = json!({"command": "git push origin main"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("リモートに送信"));
    }

    #[test]
    fn test_bash_git_pull() {
        let input = json!({"command": "git pull origin main"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("取得"));
    }

    #[test]
    fn test_bash_git_diff() {
        let input = json!({"command": "git diff HEAD"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("変更内容"));
    }

    #[test]
    fn test_bash_git_log() {
        let input = json!({"command": "git log --oneline"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("履歴"));
    }

    #[test]
    fn test_bash_git_checkout() {
        let input = json!({"command": "git checkout feature"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("ブランチ"));
    }

    #[test]
    fn test_bash_curl() {
        let input = json!({"command": "curl https://example.com"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("外部サービス"));
    }

    #[test]
    fn test_bash_npm() {
        let input = json!({"command": "npm install express"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("コマンドを実行"));
    }

    #[test]
    fn test_bash_python() {
        let input = json!({"command": "python script.py"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("Python"));
    }

    #[test]
    fn test_bash_ls() {
        let input = json!({"command": "ls"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("フォルダの中身"));
    }

    #[test]
    fn test_bash_generic() {
        let input = json!({"command": "whoami"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.description.contains("コマンドを実行"));
    }

    // ── File tools ──

    #[test]
    fn test_read_tool() {
        let input = json!({"file_path": "/home/user/docs/report.txt"});
        let t = translate_tool_event("Read", &input);
        assert!(t.description.contains("report.txt"));
        assert!(t.description.contains("読んでいます"));
    }

    #[test]
    fn test_write_tool() {
        let input = json!({"file_path": "/home/user/output.csv"});
        let t = translate_tool_event("Write", &input);
        assert!(t.description.contains("output.csv"));
        assert!(t.description.contains("作成"));
    }

    #[test]
    fn test_edit_tool() {
        let input = json!({"file_path": "C:\\Users\\test\\config.json"});
        let t = translate_tool_event("Edit", &input);
        assert!(t.description.contains("config.json"));
        assert!(t.description.contains("編集"));
    }

    #[test]
    fn test_glob_tool() {
        let input = json!({"pattern": "**/*.pdf"});
        let t = translate_tool_event("Glob", &input);
        assert!(t.description.contains("**/*.pdf"));
    }

    #[test]
    fn test_grep_tool() {
        let input = json!({"pattern": "TODO"});
        let t = translate_tool_event("Grep", &input);
        assert!(t.description.contains("TODO"));
    }

    #[test]
    fn test_todo_write() {
        let input = json!({});
        let t = translate_tool_event("TodoWrite", &input);
        assert!(t.description.contains("TODO"));
    }

    #[test]
    fn test_web_fetch() {
        let input = json!({"url": "https://example.com/page"});
        let t = translate_tool_event("WebFetch", &input);
        assert!(t.description.contains("Web"));
    }

    #[test]
    fn test_web_search() {
        let input = json!({"query": "Rust async tutorial"});
        let t = translate_tool_event("WebSearch", &input);
        assert!(t.description.contains("Rust async tutorial"));
    }

    #[test]
    fn test_task_tool() {
        let input = json!({"description": "Analyze codebase"});
        let t = translate_tool_event("Task", &input);
        assert!(t.description.contains("Analyze codebase"));
    }

    #[test]
    fn test_notebook_edit() {
        let input = json!({"notebook_path": "/home/user/analysis.ipynb"});
        let t = translate_tool_event("NotebookEdit", &input);
        assert!(t.description.contains("analysis.ipynb"));
    }

    #[test]
    fn test_unknown_tool() {
        let input = json!({});
        let t = translate_tool_event("SomeFutureTool", &input);
        assert!(t.description.contains("SomeFutureTool"));
    }

    // ── Helper functions ──

    #[test]
    fn test_extract_filename_unix() {
        assert_eq!(extract_filename("/a/b/c.txt"), "c.txt");
    }

    #[test]
    fn test_extract_filename_windows() {
        assert_eq!(extract_filename("C:\\Users\\test\\doc.pdf"), "doc.pdf");
    }

    #[test]
    fn test_extract_filename_just_name() {
        assert_eq!(extract_filename("file.rs"), "file.rs");
    }

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let result = truncate("abcdefghij", 5);
        assert_eq!(result, "abcde...");
    }

    #[test]
    fn test_truncate_unicode() {
        let result = truncate("あいうえおかきくけこ", 5);
        assert_eq!(result, "あいうえお...");
    }

    // ── raw field ──

    #[test]
    fn test_raw_contains_tool_name_and_input() {
        let input = json!({"command": "ls -la"});
        let t = translate_tool_event("Bash", &input);
        assert!(t.raw.contains("Bash"));
        assert!(t.raw.contains("ls -la"));
    }
}
