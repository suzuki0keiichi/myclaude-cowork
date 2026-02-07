import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface SlackConfig {
  bot_token: string;
  default_list_id: string | null;
}

interface GDriveConfig {
  client_id: string;
  client_secret: string;
}

type SettingsSection = "slack" | "gdrive";

export function SettingsPanel() {
  const [section, setSection] = useState<SettingsSection>("slack");

  return (
    <div style={styles.container}>
      <div style={styles.tabs}>
        <button
          style={{
            ...styles.tab,
            ...(section === "slack" ? styles.tabActive : {}),
          }}
          onClick={() => setSection("slack")}
        >
          Slack
        </button>
        <button
          style={{
            ...styles.tab,
            ...(section === "gdrive" ? styles.tabActive : {}),
          }}
          onClick={() => setSection("gdrive")}
        >
          Google Drive
        </button>
      </div>
      <div style={styles.content}>
        {section === "slack" && <SlackSettings />}
        {section === "gdrive" && <GDriveSettings />}
      </div>
    </div>
  );
}

function SlackSettings() {
  const [configured, setConfigured] = useState(false);
  const [botToken, setBotToken] = useState("");
  const [listId, setListId] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  const loadConfig = useCallback(async () => {
    try {
      const isConfigured = await invoke<boolean>("slack_is_configured");
      setConfigured(isConfigured);
      if (isConfigured) {
        const config = await invoke<SlackConfig | null>("slack_get_config");
        if (config) {
          setBotToken(config.bot_token);
          setListId(config.default_list_id || "");
        }
      }
    } catch (e) {
      console.error("Failed to load slack config:", e);
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const saveConfig = async () => {
    if (!botToken.trim()) return;
    setSaving(true);
    setMessage("");
    try {
      await invoke("slack_save_config", {
        config: {
          bot_token: botToken.trim(),
          default_list_id: listId.trim() || null,
        },
      });
      setConfigured(true);
      setMessage("保存しました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={styles.section}>
      <h3 style={styles.sectionTitle}>Slack Lists 連携</h3>
      <p style={styles.desc}>
        Slack Botトークンを設定すると、SlackリストとTODOを同期できます。
      </p>

      <label style={styles.label}>Bot Token (xoxb-...)</label>
      <input
        type="password"
        value={botToken}
        onChange={(e) => setBotToken(e.target.value)}
        placeholder="xoxb-..."
        style={styles.input}
      />

      <label style={styles.label}>デフォルトリストID (任意)</label>
      <input
        type="text"
        value={listId}
        onChange={(e) => setListId(e.target.value)}
        placeholder="L..."
        style={styles.input}
      />

      <button
        onClick={saveConfig}
        disabled={!botToken.trim() || saving}
        style={{
          ...styles.saveButton,
          opacity: botToken.trim() && !saving ? 1 : 0.5,
        }}
      >
        {saving ? "保存中..." : "設定を保存"}
      </button>

      {configured && (
        <div style={styles.statusConnected}>接続済み</div>
      )}
      {message && <div style={styles.message}>{message}</div>}
    </div>
  );
}

function GDriveSettings() {
  const [configured, setConfigured] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  const loadStatus = useCallback(async () => {
    try {
      const isConfigured = await invoke<boolean>("gdrive_is_configured");
      setConfigured(isConfigured);
      if (isConfigured) {
        const isAuth = await invoke<boolean>("gdrive_is_authenticated");
        setAuthenticated(isAuth);
      }
    } catch (e) {
      console.error("Failed to load gdrive status:", e);
    }
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const saveConfig = async () => {
    if (!clientId.trim() || !clientSecret.trim()) return;
    setSaving(true);
    setMessage("");
    try {
      await invoke("gdrive_save_config", {
        config: {
          client_id: clientId.trim(),
          client_secret: clientSecret.trim(),
        },
      });
      setConfigured(true);
      setMessage("保存しました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const startAuth = async () => {
    setMessage("");
    try {
      const url = await invoke<string>("gdrive_get_auth_url", {
        redirectPort: 8923,
      });
      // Open the auth URL in the system browser
      window.open(url, "_blank");
      setMessage("ブラウザで認証してください。認証コードをここに貼り付けてください。");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    }
  };

  const [authCode, setAuthCode] = useState("");

  const submitAuthCode = async () => {
    if (!authCode.trim()) return;
    setMessage("");
    try {
      await invoke("gdrive_exchange_code", {
        code: authCode.trim(),
        redirectPort: 8923,
      });
      setAuthenticated(true);
      setAuthCode("");
      setMessage("認証完了しました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    }
  };

  return (
    <div style={styles.section}>
      <h3 style={styles.sectionTitle}>Google Drive 連携</h3>
      <p style={styles.desc}>
        Google Cloud ConsoleでOAuth2クライアントを作成し、IDとシークレットを入力してください。
      </p>

      <label style={styles.label}>Client ID</label>
      <input
        type="text"
        value={clientId}
        onChange={(e) => setClientId(e.target.value)}
        placeholder="xxxxx.apps.googleusercontent.com"
        style={styles.input}
      />

      <label style={styles.label}>Client Secret</label>
      <input
        type="password"
        value={clientSecret}
        onChange={(e) => setClientSecret(e.target.value)}
        placeholder="GOCSPX-..."
        style={styles.input}
      />

      <button
        onClick={saveConfig}
        disabled={!clientId.trim() || !clientSecret.trim() || saving}
        style={{
          ...styles.saveButton,
          opacity:
            clientId.trim() && clientSecret.trim() && !saving ? 1 : 0.5,
        }}
      >
        {saving ? "保存中..." : "設定を保存"}
      </button>

      {configured && !authenticated && (
        <div style={styles.authSection}>
          <button onClick={startAuth} style={styles.authButton}>
            Googleアカウントで認証
          </button>
          <input
            type="text"
            value={authCode}
            onChange={(e) => setAuthCode(e.target.value)}
            placeholder="認証コードを貼り付け..."
            style={styles.input}
          />
          <button
            onClick={submitAuthCode}
            disabled={!authCode.trim()}
            style={{
              ...styles.saveButton,
              opacity: authCode.trim() ? 1 : 0.5,
            }}
          >
            認証コードを送信
          </button>
        </div>
      )}

      <div style={styles.statusRow}>
        <span>設定: {configured ? "済み" : "未設定"}</span>
        <span>認証: {authenticated ? "済み" : "未認証"}</span>
      </div>

      {message && <div style={styles.message}>{message}</div>}
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
  tabs: {
    display: "flex",
    borderBottom: "1px solid var(--border)",
  },
  tab: {
    flex: 1,
    padding: "8px 4px",
    background: "none",
    border: "none",
    borderBottom: "2px solid transparent",
    color: "var(--text-muted)",
    fontSize: "11px",
    fontFamily: "inherit",
    cursor: "pointer",
    textAlign: "center" as const,
  },
  tabActive: {
    color: "var(--text-primary)",
    borderBottomColor: "var(--accent)",
  },
  content: {
    flex: 1,
    overflowY: "auto" as const,
    padding: "12px",
  },
  section: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "8px",
  },
  sectionTitle: {
    fontSize: "14px",
    fontWeight: 600,
    color: "var(--text-primary)",
    marginBottom: "4px",
  },
  desc: {
    fontSize: "11px",
    color: "var(--text-muted)",
    lineHeight: 1.5,
    marginBottom: "8px",
  },
  label: {
    fontSize: "11px",
    color: "var(--text-secondary)",
    fontWeight: 500,
  },
  input: {
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    color: "var(--text-primary)",
    padding: "6px 8px",
    fontSize: "12px",
    fontFamily: "inherit",
    outline: "none",
    width: "100%",
  },
  saveButton: {
    background: "var(--accent)",
    border: "none",
    color: "white",
    borderRadius: "4px",
    padding: "6px 12px",
    fontSize: "12px",
    fontFamily: "inherit",
    cursor: "pointer",
    marginTop: "4px",
  },
  authSection: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "8px",
    marginTop: "8px",
    padding: "8px",
    border: "1px solid var(--border)",
    borderRadius: "6px",
  },
  authButton: {
    background: "var(--bg-tertiary)",
    border: "1px solid var(--border)",
    color: "var(--text-primary)",
    borderRadius: "4px",
    padding: "8px 12px",
    fontSize: "12px",
    fontFamily: "inherit",
    cursor: "pointer",
  },
  statusConnected: {
    fontSize: "11px",
    color: "var(--success)",
    fontWeight: 500,
  },
  statusRow: {
    display: "flex",
    gap: "16px",
    fontSize: "11px",
    color: "var(--text-muted)",
    marginTop: "4px",
  },
  message: {
    fontSize: "11px",
    color: "var(--warning)",
    marginTop: "4px",
  },
};
