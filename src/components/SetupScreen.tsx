import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";

interface SetupScreenProps {
  onSetup: (workingDir: string) => void;
  defaultPath?: string;
}

export function SetupScreen({ onSetup, defaultPath = "" }: SetupScreenProps) {
  const [path, setPath] = useState(defaultPath);
  const [showManualInput, setShowManualInput] = useState(false);

  // Update path when defaultPath arrives asynchronously
  useEffect(() => {
    if (defaultPath && !path) {
      setPath(defaultPath);
    }
  }, [defaultPath]);

  const trimmedPath = path.trim();
  const canSubmit = trimmedPath.length > 0;

  const handleSubmit = () => {
    if (canSubmit) {
      onSetup(trimmedPath);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleSubmit();
    }
  };

  const handleFolderSelect = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "作業フォルダを選択",
    });
    if (selected) {
      setPath(selected);
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <div style={styles.logo}>Cowork</div>
        <p style={styles.subtitle}>
          Claudeと一緒に作業しましょう
        </p>

        <div style={styles.form}>
          <label style={styles.label}>
            作業フォルダを選んでください
          </label>
          <p style={styles.hint}>
            Claudeが読み書きする対象のフォルダを選択してください
          </p>

          <button
            onClick={handleFolderSelect}
            style={styles.folderButton}
            type="button"
          >
            <span style={styles.folderIcon} role="img" aria-label="フォルダ">
              📁
            </span>
            <span>フォルダを選択</span>
          </button>

          {path && (
            <div style={styles.selectedPath}>
              <span style={styles.selectedPathLabel}>選択中:</span>
              <span style={styles.selectedPathValue}>{path}</span>
            </div>
          )}

          <div style={styles.manualToggleArea}>
            <button
              onClick={() => setShowManualInput(!showManualInput)}
              style={styles.manualToggle}
              type="button"
            >
              {showManualInput ? "手動入力を閉じる" : "または手動で入力"}
            </button>
          </div>

          {showManualInput && (
            <input
              type="text"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="例: C:\Users\ユーザー名\Documents\仕事"
              style={styles.input}
            />
          )}

          <button
            onClick={handleSubmit}
            disabled={!canSubmit}
            style={{
              ...styles.button,
              opacity: canSubmit ? 1 : 0.5,
              cursor: canSubmit ? "pointer" : "default",
            }}
          >
            はじめる
          </button>
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    height: "100%",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    background: "var(--bg-primary)",
  },
  card: {
    background: "var(--bg-secondary)",
    borderRadius: "16px",
    padding: "48px",
    maxWidth: "480px",
    width: "100%",
    textAlign: "center",
    border: "1px solid var(--border)",
  },
  logo: {
    fontSize: "32px",
    fontWeight: 700,
    color: "var(--accent)",
    marginBottom: "8px",
    letterSpacing: "-0.5px",
  },
  subtitle: {
    color: "var(--text-secondary)",
    fontSize: "15px",
    marginBottom: "32px",
  },
  form: {
    textAlign: "left",
  },
  label: {
    display: "block",
    fontSize: "14px",
    fontWeight: 600,
    marginBottom: "4px",
    color: "var(--text-primary)",
  },
  hint: {
    fontSize: "12px",
    color: "var(--text-muted)",
    marginBottom: "16px",
  },
  folderButton: {
    width: "100%",
    padding: "20px",
    background: "var(--bg-input)",
    border: "2px dashed var(--border)",
    borderRadius: "12px",
    color: "var(--text-primary)",
    fontSize: "16px",
    fontWeight: 600,
    fontFamily: "inherit",
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "10px",
    transition: "border-color 0.2s, background 0.2s",
  },
  folderIcon: {
    fontSize: "24px",
  },
  selectedPath: {
    marginTop: "12px",
    padding: "10px 14px",
    background: "var(--bg-input)",
    borderRadius: "8px",
    border: "1px solid var(--border)",
    wordBreak: "break-all",
  },
  selectedPathLabel: {
    fontSize: "11px",
    color: "var(--text-muted)",
    display: "block",
    marginBottom: "4px",
  },
  selectedPathValue: {
    fontSize: "13px",
    color: "var(--text-secondary)",
    fontFamily: "monospace",
  },
  manualToggleArea: {
    textAlign: "center",
    marginTop: "16px",
    marginBottom: "16px",
  },
  manualToggle: {
    background: "none",
    border: "none",
    color: "var(--text-muted)",
    fontSize: "12px",
    cursor: "pointer",
    textDecoration: "underline",
    fontFamily: "inherit",
    padding: "4px 8px",
  },
  input: {
    width: "100%",
    padding: "10px 14px",
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "8px",
    color: "var(--text-primary)",
    fontSize: "14px",
    fontFamily: "inherit",
    outline: "none",
    marginBottom: "16px",
  },
  button: {
    width: "100%",
    padding: "12px",
    background: "var(--accent)",
    color: "white",
    border: "none",
    borderRadius: "8px",
    fontSize: "15px",
    fontWeight: 600,
    cursor: "pointer",
  },
};
