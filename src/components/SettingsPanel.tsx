import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import type { LocalLlmSettings } from "../types";

export function SettingsPanel() {
  return (
    <div style={styles.container}>
      <div style={styles.content}>
        <LocalLlmSettingsPanel />
        <div style={styles.divider} />
        <GDriveSettings />
        <div style={styles.divider} />
        <SlackSettings />
      </div>
    </div>
  );
}

// ── Local LLM (OpenVINO / OpenAI-compatible) ──

function LocalLlmSettingsPanel() {
  const [settings, setSettings] = useState<LocalLlmSettings>({
    enabled: false,
    endpoint: "http://localhost:8000/v1",
    model: "default",
    api_key: null,
    system_prompt: null,
  });
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [message, setMessage] = useState("");

  const loadSettings = useCallback(async () => {
    try {
      const s = await invoke<LocalLlmSettings>("local_llm_get_settings");
      setSettings(s);
    } catch (e) {
      console.error("Failed to load local LLM settings:", e);
    }
  }, []);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const save = async () => {
    setSaving(true);
    setMessage("");
    try {
      await invoke("local_llm_save_settings", { settings });
      setMessage("保存しました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const testConnection = async () => {
    setTesting(true);
    setMessage("");
    try {
      // Save first so the backend uses latest settings
      await invoke("local_llm_save_settings", { settings });
      const result = await invoke<string>("local_llm_test_connection");
      setMessage(result);
    } catch (e) {
      setMessage(`${e}`);
    } finally {
      setTesting(false);
    }
  };

  return (
    <div style={styles.section}>
      <h3 style={styles.sectionTitle}>ローカルLLM（OpenVINO / Ollama等対応）</h3>
      <p style={styles.desc}>
        OpenVINO Model Server、Ollama、llama.cpp、vLLMなど、OpenAI互換APIを提供するローカルLLMサーバーと連携します。
      </p>

      <label style={styles.label}>
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={(e) =>
            setSettings({ ...settings, enabled: e.target.checked })
          }
          style={{ marginRight: "6px" }}
        />
        ローカルLLMを有効にする
      </label>

      <label style={styles.label}>APIエンドポイント</label>
      <input
        type="text"
        value={settings.endpoint}
        onChange={(e) =>
          setSettings({ ...settings, endpoint: e.target.value })
        }
        placeholder="http://localhost:8000/v1"
        style={styles.input}
      />

      <label style={styles.label}>モデル名</label>
      <input
        type="text"
        value={settings.model}
        onChange={(e) =>
          setSettings({ ...settings, model: e.target.value })
        }
        placeholder="default"
        style={styles.input}
      />

      <label style={styles.label}>APIキー（任意）</label>
      <input
        type="password"
        value={settings.api_key || ""}
        onChange={(e) =>
          setSettings({
            ...settings,
            api_key: e.target.value || null,
          })
        }
        placeholder="不要な場合は空欄"
        style={styles.input}
      />

      <label style={styles.label}>システムプロンプト（任意）</label>
      <textarea
        value={settings.system_prompt || ""}
        onChange={(e) =>
          setSettings({
            ...settings,
            system_prompt: e.target.value || null,
          })
        }
        placeholder="例: あなたは親切なアシスタントです"
        rows={3}
        style={{ ...styles.input, resize: "vertical" as const, fontFamily: "inherit" }}
      />

      <div style={{ display: "flex", gap: "8px", marginTop: "4px" }}>
        <button
          onClick={save}
          disabled={saving}
          style={{
            ...styles.saveButton,
            opacity: saving ? 0.5 : 1,
          }}
        >
          {saving ? "保存中..." : "保存"}
        </button>
        <button
          onClick={testConnection}
          disabled={testing}
          style={{
            ...styles.authButton,
            opacity: testing ? 0.6 : 1,
            background: "var(--text-secondary)",
          }}
        >
          {testing ? "テスト中..." : "接続テスト"}
        </button>
      </div>

      {message && <div style={styles.message}>{message}</div>}
    </div>
  );
}

// ── Google Drive ──

function GDriveSettings() {
  const [configured, setConfigured] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [loading, setLoading] = useState(false);
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

    const unlisteners = [
      listen("gdrive:auth_complete", () => {
        setAuthenticated(true);
        setLoading(false);
        setMessage("認証が完了しました");
      }),
      listen("gdrive:auth_error", (e) => {
        setLoading(false);
        setMessage(`認証エラー: ${e.payload}`);
      }),
    ];

    return () => {
      unlisteners.forEach((p) => p.then((f) => f()));
    };
  }, [loadStatus]);

  const startAuth = async () => {
    setLoading(true);
    setMessage("ブラウザで認証してください...");
    try {
      const url = await invoke<string>("gdrive_start_auth");
      await open(url);
    } catch (e) {
      setLoading(false);
      setMessage(`エラー: ${e}`);
    }
  };

  const logout = async () => {
    try {
      await invoke("gdrive_logout");
      setAuthenticated(false);
      setMessage("ログアウトしました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    }
  };

  return (
    <div style={styles.section}>
      <h3 style={styles.sectionTitle}>Google Drive 連携</h3>

      {!configured ? (
        <p style={styles.desc}>
          Google
          Drive連携の設定がアプリに含まれていません。開発者に連絡してください。
        </p>
      ) : !authenticated ? (
        <div style={styles.authRow}>
          <button
            onClick={startAuth}
            disabled={loading}
            style={{
              ...styles.authButton,
              opacity: loading ? 0.6 : 1,
            }}
          >
            {loading ? "認証中..." : "Googleでログイン"}
          </button>
        </div>
      ) : (
        <div style={styles.authRow}>
          <span style={styles.statusOk}>認証済み</span>
          <button onClick={logout} style={styles.logoutButton}>
            ログアウト
          </button>
        </div>
      )}

      {message && <div style={styles.message}>{message}</div>}
    </div>
  );
}

// ── Slack ──

function SlackSettings() {
  const [configured, setConfigured] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const [teamName, setTeamName] = useState("");
  const [listId, setListId] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState("");

  const loadStatus = useCallback(async () => {
    try {
      const isConfigured = await invoke<boolean>("slack_is_configured");
      setConfigured(isConfigured);
      if (isConfigured) {
        const isAuth = await invoke<boolean>("slack_is_authenticated");
        setAuthenticated(isAuth);
        if (isAuth) {
          const name = await invoke<string | null>("slack_get_team_name");
          setTeamName(name || "");
        }
        const settings = await invoke<{ default_list_id: string | null }>(
          "slack_get_settings"
        );
        setListId(settings.default_list_id || "");
      }
    } catch (e) {
      console.error("Failed to load slack status:", e);
    }
  }, []);

  useEffect(() => {
    loadStatus();

    const unlisteners = [
      listen("slack:auth_complete", () => {
        setAuthenticated(true);
        setLoading(false);
        setMessage("認証が完了しました");
        // Reload to get team name
        invoke<string | null>("slack_get_team_name").then((name) =>
          setTeamName(name || "")
        );
      }),
      listen("slack:auth_error", (e) => {
        setLoading(false);
        setMessage(`認証エラー: ${e.payload}`);
      }),
    ];

    return () => {
      unlisteners.forEach((p) => p.then((f) => f()));
    };
  }, [loadStatus]);

  const startAuth = async () => {
    setLoading(true);
    setMessage("ブラウザで認証してください...");
    try {
      const url = await invoke<string>("slack_start_auth");
      await open(url);
    } catch (e) {
      setLoading(false);
      setMessage(`エラー: ${e}`);
    }
  };

  const logout = async () => {
    try {
      await invoke("slack_logout");
      setAuthenticated(false);
      setTeamName("");
      setMessage("ログアウトしました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    }
  };

  const saveSettings = async () => {
    setSaving(true);
    setMessage("");
    try {
      await invoke("slack_save_settings", {
        settings: { default_list_id: listId.trim() || null },
      });
      setMessage("保存しました");
    } catch (e) {
      setMessage(`エラー: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div style={styles.section}>
      <h3 style={styles.sectionTitle}>Slack 連携</h3>

      {!configured ? (
        <p style={styles.desc}>
          Slack連携の設定がアプリに含まれていません。開発者に連絡してください。
        </p>
      ) : !authenticated ? (
        <div style={styles.authRow}>
          <button
            onClick={startAuth}
            disabled={loading}
            style={{
              ...styles.authButton,
              opacity: loading ? 0.6 : 1,
            }}
          >
            {loading ? "認証中..." : "Slackに追加"}
          </button>
        </div>
      ) : (
        <div style={styles.configSection}>
          <div style={styles.authRow}>
            <span style={styles.statusOk}>
              認証済み{teamName ? ` (${teamName})` : ""}
            </span>
            <button onClick={logout} style={styles.logoutButton}>
              ログアウト
            </button>
          </div>

          <label style={styles.label}>リストID</label>
          <input
            type="text"
            value={listId}
            onChange={(e) => setListId(e.target.value)}
            placeholder="L..."
            style={styles.input}
          />
          <button
            onClick={saveSettings}
            disabled={saving}
            style={{
              ...styles.saveButton,
              opacity: saving ? 0.5 : 1,
            }}
          >
            {saving ? "保存中..." : "保存"}
          </button>
        </div>
      )}

      {message && <div style={styles.message}>{message}</div>}
    </div>
  );
}

// ── Styles ──

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  content: {
    flex: 1,
    overflowY: "auto",
    padding: "12px",
  },
  section: {
    display: "flex",
    flexDirection: "column",
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
  },
  divider: {
    height: "1px",
    background: "var(--border)",
    margin: "16px 0",
  },
  authRow: {
    display: "flex",
    alignItems: "center",
    gap: "12px",
  },
  authButton: {
    background: "var(--accent)",
    border: "none",
    color: "white",
    borderRadius: "4px",
    padding: "8px 16px",
    fontSize: "12px",
    fontFamily: "inherit",
    cursor: "pointer",
    fontWeight: 500,
  },
  statusOk: {
    fontSize: "12px",
    color: "var(--success, #22c55e)",
    fontWeight: 500,
  },
  logoutButton: {
    background: "none",
    border: "1px solid var(--border)",
    color: "var(--text-muted)",
    borderRadius: "4px",
    padding: "4px 10px",
    fontSize: "11px",
    fontFamily: "inherit",
    cursor: "pointer",
  },
  configSection: {
    display: "flex",
    flexDirection: "column",
    gap: "8px",
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
    alignSelf: "flex-start",
  },
  message: {
    fontSize: "11px",
    color: "var(--warning)",
    marginTop: "4px",
  },
};
