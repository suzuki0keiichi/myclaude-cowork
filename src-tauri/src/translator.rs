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
