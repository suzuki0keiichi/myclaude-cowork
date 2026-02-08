import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface CoworkCommand {
  name: string;
  description: string;
  body: string;
}

interface CommandManagerProps {
  workingDir: string;
  selectedFiles: string[];
  onExecute: (message: string) => void;
}

export function CommandManager({ workingDir, selectedFiles, onExecute }: CommandManagerProps) {
  const [commands, setCommands] = useState<CoworkCommand[]>([]);
  const [selectedCommand, setSelectedCommand] = useState<CoworkCommand | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [additionalInput, setAdditionalInput] = useState("");
  const [newCommand, setNewCommand] = useState({
    name: "",
    description: "",
    body: "",
  });

  const loadCommands = useCallback(async () => {
    try {
      const items = await invoke<CoworkCommand[]>("list_commands");
      setCommands(items);
    } catch (e) {
      console.error("Failed to load commands:", e);
    }
  }, []);

  useEffect(() => {
    loadCommands();
  }, [loadCommands, workingDir]);

  const buildContext = (): string => {
    const parts: string[] = [];

    if (selectedFiles.length > 0) {
      parts.push("対象ファイル:");
      parts.push(...selectedFiles.map((f) => `- ${f}`));
    }

    if (additionalInput.trim()) {
      parts.push(additionalInput.trim());
    }

    return parts.join("\n");
  };

  const executeCommand = () => {
    if (!selectedCommand) return;
    const context = buildContext();
    const message = context
      ? `/${selectedCommand.name} ${context}`
      : `/${selectedCommand.name}`;
    onExecute(message);
    setSelectedCommand(null);
    setAdditionalInput("");
  };

  const saveNewCommand = async () => {
    if (!newCommand.name.trim() || !newCommand.body.trim()) return;

    const command: CoworkCommand = {
      name: newCommand.name.trim(),
      description: newCommand.description.trim(),
      body: newCommand.body.trim(),
    };

    try {
      await invoke("save_command", { command });
      setShowForm(false);
      setNewCommand({ name: "", description: "", body: "" });
      await loadCommands();
    } catch (e) {
      console.error("Failed to save command:", e);
    }
  };

  const deleteCommand = async (name: string) => {
    try {
      await invoke("delete_command", { name });
      if (selectedCommand?.name === name) setSelectedCommand(null);
      await loadCommands();
    } catch (e) {
      console.error("Failed to delete command:", e);
    }
  };

  // Command execution dialog
  if (selectedCommand) {
    return (
      <div style={styles.container}>
        <div style={styles.header}>
          <button onClick={() => setSelectedCommand(null)} style={styles.backButton}>
            ← 戻る
          </button>
          <span style={styles.headerTitle}>{selectedCommand.name}</span>
        </div>
        <div style={styles.content}>
          <p style={styles.description}>{selectedCommand.description}</p>

          <div style={styles.contextSection}>
            <label style={styles.paramLabel}>自動提供されるコンテキスト</label>
            <div style={styles.contextBox}>
              <div style={styles.contextItem}>
                <span style={styles.contextLabel}>作業フォルダ:</span>
                <span style={styles.contextValue}>{workingDir || "未設定"}</span>
              </div>
              {selectedFiles.length > 0 && (
                <div style={styles.contextItem}>
                  <span style={styles.contextLabel}>選択ファイル:</span>
                  <div>
                    {selectedFiles.map((f, i) => (
                      <div key={i} style={styles.contextValue}>
                        {f.split(/[\\/]/).pop()}
                      </div>
                    ))}
                  </div>
                </div>
              )}
              {selectedFiles.length === 0 && (
                <div style={styles.contextHint}>
                  ファイルタブでファイルを選択すると、コマンドに自動で渡されます
                </div>
              )}
            </div>
          </div>

          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>追加の指示（任意）</label>
            <textarea
              value={additionalInput}
              onChange={(e) => setAdditionalInput(e.target.value)}
              style={{ ...styles.paramInput, minHeight: "60px", resize: "vertical" as const }}
              placeholder="例: 2024年のファイルだけ対象にして"
            />
          </div>

          <button onClick={executeCommand} style={styles.executeButton}>
            実行する
          </button>
        </div>
      </div>
    );
  }

  // New command form
  if (showForm) {
    return (
      <div style={styles.container}>
        <div style={styles.header}>
          <button onClick={() => setShowForm(false)} style={styles.backButton}>
            ← 戻る
          </button>
          <span style={styles.headerTitle}>新しいコマンド</span>
        </div>
        <div style={styles.content}>
          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>コマンド名</label>
            <input
              type="text"
              value={newCommand.name}
              onChange={(e) => setNewCommand({ ...newCommand, name: e.target.value })}
              style={styles.paramInput}
              placeholder="例: 請求書振り分け"
            />
          </div>
          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>説明</label>
            <input
              type="text"
              value={newCommand.description}
              onChange={(e) =>
                setNewCommand({ ...newCommand, description: e.target.value })
              }
              style={styles.paramInput}
              placeholder="例: PDFの請求書を取引先ごとに振り分ける"
            />
          </div>
          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>指示内容</label>
            <textarea
              value={newCommand.body}
              onChange={(e) =>
                setNewCommand({ ...newCommand, body: e.target.value })
              }
              style={{ ...styles.paramInput, minHeight: "120px", resize: "vertical" as const }}
              placeholder={
                "例:\n請求書のPDFファイルを取引先ごとにフォルダに振り分けてください。\n\n" +
                "ルール:\n- 「株式会社A」を含むファイル → A社フォルダ\n- 「B商事」を含むファイル → B商事フォルダ\n\n" +
                "$ARGUMENTS"
              }
            />
            <p style={styles.hint}>
              $ARGUMENTS と書くと、実行時に選択ファイルや追加指示が自動で入ります
            </p>
          </div>
          <button
            onClick={saveNewCommand}
            disabled={!newCommand.name.trim() || !newCommand.body.trim()}
            style={{
              ...styles.executeButton,
              opacity: newCommand.name.trim() && newCommand.body.trim() ? 1 : 0.4,
            }}
          >
            保存
          </button>
        </div>
      </div>
    );
  }

  // No working directory set
  if (!workingDir) {
    return (
      <div style={styles.container}>
        <div style={styles.empty}>
          作業フォルダを選択すると
          <br />
          コマンドが表示されます。
        </div>
      </div>
    );
  }

  // Command list
  return (
    <div style={styles.container}>
      <div style={styles.list}>
        {commands.map((cmd) => (
          <div key={cmd.name} style={styles.commandItem}>
            <div
              style={styles.commandInfo}
              onClick={() => setSelectedCommand(cmd)}
            >
              <div style={styles.commandName}>{cmd.name}</div>
              <div style={styles.commandDesc}>{cmd.description}</div>
            </div>
            <button
              onClick={() => deleteCommand(cmd.name)}
              style={styles.commandDelete}
              title="削除"
            >
              ×
            </button>
          </div>
        ))}
        {commands.length === 0 && (
          <div style={styles.empty}>
            コマンドはまだありません。
            <br />
            よく使う操作を登録しましょう。
          </div>
        )}
      </div>
      <div style={styles.footer}>
        <button onClick={() => setShowForm(true)} style={styles.newButton}>
          + 新しいコマンド
        </button>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  header: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "8px 12px",
    borderBottom: "1px solid var(--border)",
  },
  headerTitle: {
    fontSize: "13px",
    fontWeight: 600,
  },
  backButton: {
    background: "none",
    border: "none",
    color: "var(--text-secondary)",
    cursor: "pointer",
    fontSize: "12px",
    padding: "2px 4px",
    fontFamily: "inherit",
  },
  content: {
    flex: 1,
    padding: "12px",
    overflowY: "auto",
  },
  description: {
    fontSize: "12px",
    color: "var(--text-secondary)",
    marginBottom: "12px",
  },
  contextSection: {
    marginBottom: "12px",
  },
  contextBox: {
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    padding: "8px",
    fontSize: "11px",
  },
  contextItem: {
    display: "flex",
    gap: "6px",
    marginBottom: "4px",
  },
  contextLabel: {
    color: "var(--text-muted)",
    whiteSpace: "nowrap",
  },
  contextValue: {
    color: "var(--text-primary)",
    wordBreak: "break-all",
  },
  contextHint: {
    color: "var(--text-muted)",
    fontStyle: "italic",
    fontSize: "11px",
  },
  paramGroup: {
    marginBottom: "10px",
  },
  paramLabel: {
    display: "block",
    fontSize: "12px",
    fontWeight: 600,
    marginBottom: "4px",
    color: "var(--text-secondary)",
  },
  paramInput: {
    width: "100%",
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    color: "var(--text-primary)",
    padding: "6px 8px",
    fontSize: "12px",
    fontFamily: "inherit",
    outline: "none",
  },
  hint: {
    fontSize: "11px",
    color: "var(--text-muted)",
    marginTop: "4px",
  },
  executeButton: {
    width: "100%",
    padding: "8px",
    background: "var(--accent)",
    color: "white",
    border: "none",
    borderRadius: "6px",
    fontSize: "13px",
    fontWeight: 600,
    cursor: "pointer",
    marginTop: "8px",
  },
  list: {
    flex: 1,
    overflowY: "auto",
    padding: "4px 0",
  },
  commandItem: {
    display: "flex",
    alignItems: "center",
    padding: "8px 12px",
    borderBottom: "1px solid var(--border)",
    cursor: "pointer",
  },
  commandInfo: {
    flex: 1,
    overflow: "hidden",
  },
  commandName: {
    fontSize: "13px",
    fontWeight: 600,
    color: "var(--text-primary)",
  },
  commandDesc: {
    fontSize: "11px",
    color: "var(--text-muted)",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  commandDelete: {
    background: "none",
    border: "none",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "16px",
    padding: "0 4px",
    opacity: 0.5,
  },
  empty: {
    padding: "20px 12px",
    textAlign: "center",
    color: "var(--text-muted)",
    fontSize: "12px",
    lineHeight: "1.6",
  },
  footer: {
    padding: "8px 12px",
    borderTop: "1px solid var(--border)",
  },
  newButton: {
    width: "100%",
    padding: "6px",
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "6px",
    color: "var(--text-secondary)",
    fontSize: "12px",
    cursor: "pointer",
    fontFamily: "inherit",
  },
};
