import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface CoworkSkill {
  name: string;
  description: string;
  body: string;
}

interface SkillManagerProps {
  workingDir: string;
  selectedFiles: string[];
  onExecute: (message: string) => void;
}

export function SkillManager({ workingDir, selectedFiles, onExecute }: SkillManagerProps) {
  const [skills, setSkills] = useState<CoworkSkill[]>([]);
  const [selectedSkill, setSelectedSkill] = useState<CoworkSkill | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [additionalInput, setAdditionalInput] = useState("");
  const [newSkill, setNewSkill] = useState({
    name: "",
    description: "",
    body: "",
  });

  const loadSkills = useCallback(async () => {
    try {
      const items = await invoke<CoworkSkill[]>("list_skills");
      setSkills(items);
    } catch (e) {
      console.error("Failed to load skills:", e);
    }
  }, []);

  useEffect(() => {
    loadSkills();
  }, [loadSkills, workingDir]);

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

  const executeSkill = async () => {
    if (!selectedSkill) return;
    const context = buildContext();
    try {
      await invoke("execute_skill", { name: selectedSkill.name, context });
    } catch (e) {
      console.error("Failed to execute skill:", e);
    }
    setSelectedSkill(null);
    setAdditionalInput("");
  };

  const saveNewSkill = async () => {
    if (!newSkill.name.trim() || !newSkill.body.trim()) return;

    const skill: CoworkSkill = {
      name: newSkill.name.trim(),
      description: newSkill.description.trim(),
      body: newSkill.body.trim(),
    };

    try {
      await invoke("save_skill", { skill });
      setShowForm(false);
      setNewSkill({ name: "", description: "", body: "" });
      await loadSkills();
    } catch (e) {
      console.error("Failed to save skill:", e);
    }
  };

  const deleteSkill = async (name: string) => {
    try {
      await invoke("delete_skill", { name });
      if (selectedSkill?.name === name) setSelectedSkill(null);
      await loadSkills();
    } catch (e) {
      console.error("Failed to delete skill:", e);
    }
  };

  const startChatCreation = () => {
    const prompt = [
      "ユーザーが繰り返し使えるスキル（自動化手順）を新しく作りたいと言っています。",
      "以下の手順で対話的にスキルを作成してください。",
      "",
      "## 進め方",
      "",
      "1. まずユーザーに「どんな作業を自動化したいですか？」と聞いてください。",
      "2. ユーザーの回答をもとに、あなたの理解を **Mermaid形式のフローチャート** で示してください。",
      "   - ```mermaid で囲んでください（このアプリはMermaid図を描画できます）",
      "   - フローチャートには処理の流れ、条件分岐、入出力を含めてください",
      "   - ノードのラベルは日本語で書いてください",
      "3. 「この理解で合っていますか？修正したい点があれば教えてください」と確認してください。",
      "4. ユーザーがOKと言うまで 2-3 を繰り返してください。",
      "5. 確定したら、スキルファイルをWriteツールで書き込んでください。",
      "   - パス: .claude/skills/{スキル名}/SKILL.md",
      "   - スキル名は日本語OK（例: .claude/skills/請求書振り分け/SKILL.md）",
      "   - YAML frontmatter に name と description を入れてください",
      "   - 本文は、Claudeへの指示として機能するプロンプトにしてください",
      "   - 実行時にファイルリストなど追加情報が渡される場合は $ARGUMENTS を使ってください",
      "",
      "## ファイル形式",
      "",
      "```",
      "---",
      "name: スキル名",
      "description: このスキルの説明",
      "---",
      "",
      "ここにClaude への指示を書く",
      "",
      "$ARGUMENTS",
      "```",
      "",
      "## 注意",
      "",
      "- ユーザーはプログラミングの知識がありません。専門用語は避けてください。",
      "- フローチャートで「こういうことですよね？」と確認するのが最も重要なステップです。",
      "- スキル名は分かりやすい日本語にしてください。",
    ].join("\n");

    onExecute(prompt);
  };

  // Skill execution dialog
  if (selectedSkill) {
    return (
      <div style={styles.container}>
        <div style={styles.header}>
          <button onClick={() => setSelectedSkill(null)} style={styles.backButton}>
            ← 戻る
          </button>
          <span style={styles.headerTitle}>{selectedSkill.name}</span>
        </div>
        <div style={styles.content}>
          <p style={styles.description}>{selectedSkill.description}</p>

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
                  ファイルタブでファイルを選択すると、スキルに自動で渡されます
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

          <button onClick={executeSkill} style={styles.executeButton}>
            実行する
          </button>
        </div>
      </div>
    );
  }

  // New skill form
  if (showForm) {
    return (
      <div style={styles.container}>
        <div style={styles.header}>
          <button onClick={() => setShowForm(false)} style={styles.backButton}>
            ← 戻る
          </button>
          <span style={styles.headerTitle}>新しいスキル</span>
        </div>
        <div style={styles.content}>
          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>スキル名</label>
            <input
              type="text"
              value={newSkill.name}
              onChange={(e) => setNewSkill({ ...newSkill, name: e.target.value })}
              style={styles.paramInput}
              placeholder="例: 請求書振り分け"
            />
          </div>
          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>説明</label>
            <input
              type="text"
              value={newSkill.description}
              onChange={(e) =>
                setNewSkill({ ...newSkill, description: e.target.value })
              }
              style={styles.paramInput}
              placeholder="例: PDFの請求書を取引先ごとに振り分ける"
            />
          </div>
          <div style={styles.paramGroup}>
            <label style={styles.paramLabel}>指示内容</label>
            <textarea
              value={newSkill.body}
              onChange={(e) =>
                setNewSkill({ ...newSkill, body: e.target.value })
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
            onClick={saveNewSkill}
            disabled={!newSkill.name.trim() || !newSkill.body.trim()}
            style={{
              ...styles.executeButton,
              opacity: newSkill.name.trim() && newSkill.body.trim() ? 1 : 0.4,
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
          スキルが表示されます。
        </div>
      </div>
    );
  }

  // Skill list
  return (
    <div style={styles.container}>
      <div style={styles.list}>
        {skills.map((skill) => (
          <div key={skill.name} style={styles.skillItem}>
            <div
              style={styles.skillInfo}
              onClick={() => setSelectedSkill(skill)}
            >
              <div style={styles.skillName}>{skill.name}</div>
              <div style={styles.skillDesc}>{skill.description}</div>
            </div>
            <button
              onClick={() => deleteSkill(skill.name)}
              style={styles.skillDelete}
              title="削除"
            >
              ×
            </button>
          </div>
        ))}
        {skills.length === 0 && (
          <div style={styles.empty}>
            スキルはまだありません。
            <br />
            「チャットで作る」から、やりたいことを
            <br />
            話すだけでスキルを作れます。
          </div>
        )}
      </div>
      <div style={styles.footer}>
        <button onClick={startChatCreation} style={styles.chatCreateButton}>
          + チャットで作る
        </button>
        <button onClick={() => setShowForm(true)} style={styles.manualCreateButton}>
          自分で書く
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
  skillItem: {
    display: "flex",
    alignItems: "center",
    padding: "8px 12px",
    borderBottom: "1px solid var(--border)",
    cursor: "pointer",
  },
  skillInfo: {
    flex: 1,
    overflow: "hidden",
  },
  skillName: {
    fontSize: "13px",
    fontWeight: 600,
    color: "var(--text-primary)",
  },
  skillDesc: {
    fontSize: "11px",
    color: "var(--text-muted)",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  skillDelete: {
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
  chatCreateButton: {
    width: "100%",
    padding: "8px",
    background: "var(--accent)",
    border: "none",
    borderRadius: "6px",
    color: "white",
    fontSize: "12px",
    fontWeight: 600,
    cursor: "pointer",
    fontFamily: "inherit",
    marginBottom: "4px",
  },
  manualCreateButton: {
    width: "100%",
    padding: "6px",
    background: "none",
    border: "none",
    borderRadius: "6px",
    color: "var(--text-muted)",
    fontSize: "11px",
    cursor: "pointer",
    fontFamily: "inherit",
    textDecoration: "underline" as const,
  },
};
