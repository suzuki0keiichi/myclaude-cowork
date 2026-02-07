import { useState } from "react";

interface SetupScreenProps {
  onSetup: (workingDir: string) => void;
}

export function SetupScreen({ onSetup }: SetupScreenProps) {
  const [path, setPath] = useState("");

  const handleSubmit = () => {
    if (path.trim()) {
      onSetup(path.trim());
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      handleSubmit();
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
            Claudeが読み書きする対象のフォルダのパスを入力してください
          </p>
          <input
            type="text"
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="例: C:\Users\ユーザー名\Documents\仕事"
            style={styles.input}
            autoFocus
          />
          <button
            onClick={handleSubmit}
            disabled={!path.trim()}
            style={{
              ...styles.button,
              opacity: path.trim() ? 1 : 0.5,
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
    marginBottom: "12px",
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
