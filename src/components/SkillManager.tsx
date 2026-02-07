import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SkillParam {
  name: string;
  label: string;
  param_type: string;
  default_value: string;
  options: string[];
}

interface Skill {
  id: string;
  name: string;
  description: string;
  prompt_template: string;
  parameters: SkillParam[];
  created_at: string;
  updated_at: string;
}

interface SkillManagerProps {
  onExecuteSkill: (prompt: string) => void;
}

export function SkillManager({ onExecuteSkill: _onExecuteSkill }: SkillManagerProps) {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [selectedSkill, setSelectedSkill] = useState<Skill | null>(null);
  const [paramValues, setParamValues] = useState<Record<string, string>>({});
  const [showForm, setShowForm] = useState(false);
  const [newSkill, setNewSkill] = useState({
    name: "",
    description: "",
    prompt_template: "",
  });

  const loadSkills = useCallback(async () => {
    try {
      const items = await invoke<Skill[]>("list_skills");
      setSkills(items);
    } catch (e) {
      console.error("Failed to load skills:", e);
    }
  }, []);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const selectSkill = (skill: Skill) => {
    setSelectedSkill(skill);
    const defaults: Record<string, string> = {};
    skill.parameters.forEach((p) => {
      defaults[p.name] = p.default_value;
    });
    setParamValues(defaults);
  };

  const executeSkill = async () => {
    if (!selectedSkill) return;
    try {
      await invoke("execute_skill", {
        skillId: selectedSkill.id,
        params: paramValues,
      });
      setSelectedSkill(null);
    } catch (e) {
      console.error("Failed to execute skill:", e);
    }
  };

  const saveNewSkill = async () => {
    if (!newSkill.name.trim() || !newSkill.prompt_template.trim()) return;

    // Extract {{param}} placeholders from template
    const paramMatches = newSkill.prompt_template.match(/\{\{(\w+)\}\}/g) || [];
    const paramNames = [...new Set(paramMatches.map((m) => m.slice(2, -2)))];

    const skill: Skill = {
      id: crypto.randomUUID(),
      name: newSkill.name.trim(),
      description: newSkill.description.trim(),
      prompt_template: newSkill.prompt_template.trim(),
      parameters: paramNames.map((name) => ({
        name,
        label: name,
        param_type: "text",
        default_value: "",
        options: [],
      })),
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    try {
      await invoke("save_skill", { skill });
      setShowForm(false);
      setNewSkill({ name: "", description: "", prompt_template: "" });
      await loadSkills();
    } catch (e) {
      console.error("Failed to save skill:", e);
    }
  };

  const deleteSkill = async (id: string) => {
    try {
      await invoke("delete_skill", { id });
      if (selectedSkill?.id === id) setSelectedSkill(null);
      await loadSkills();
    } catch (e) {
      console.error("Failed to delete skill:", e);
    }
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
          {selectedSkill.parameters.map((param) => (
            <div key={param.name} style={styles.paramGroup}>
              <label style={styles.paramLabel}>{param.label}</label>
              <input
                type="text"
                value={paramValues[param.name] || ""}
                onChange={(e) =>
                  setParamValues({ ...paramValues, [param.name]: e.target.value })
                }
                style={styles.paramInput}
                placeholder={`${param.label}を入力...`}
              />
            </div>
          ))}
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
            <label style={styles.paramLabel}>プロンプト</label>
            <textarea
              value={newSkill.prompt_template}
              onChange={(e) =>
                setNewSkill({ ...newSkill, prompt_template: e.target.value })
              }
              style={{ ...styles.paramInput, minHeight: "80px", resize: "vertical" as const }}
              placeholder={"例: {{folder}}の中のPDFファイルを\n取引先ごとにフォルダ振り分けして"}
            />
            <p style={styles.hint}>
              {"{{名前}}"} でパラメータを定義できます
            </p>
          </div>
          <button
            onClick={saveNewSkill}
            disabled={!newSkill.name.trim() || !newSkill.prompt_template.trim()}
            style={{
              ...styles.executeButton,
              opacity: newSkill.name.trim() && newSkill.prompt_template.trim() ? 1 : 0.4,
            }}
          >
            保存
          </button>
        </div>
      </div>
    );
  }

  // Skill list
  return (
    <div style={styles.container}>
      <div style={styles.list}>
        {skills.map((skill) => (
          <div key={skill.id} style={styles.skillItem}>
            <div
              style={styles.skillInfo}
              onClick={() => selectSkill(skill)}
            >
              <div style={styles.skillName}>{skill.name}</div>
              <div style={styles.skillDesc}>{skill.description}</div>
            </div>
            <button
              onClick={() => deleteSkill(skill.id)}
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
            よく使う操作を保存しましょう。
          </div>
        )}
      </div>
      <div style={styles.footer}>
        <button onClick={() => setShowForm(true)} style={styles.newButton}>
          + 新しいスキル
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
