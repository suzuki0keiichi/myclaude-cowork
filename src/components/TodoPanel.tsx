import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface TodoItem {
  id: string;
  text: string;
  done: boolean;
  created_at: string;
  due_date: string | null;
}

export function TodoPanel() {
  const [items, setItems] = useState<TodoItem[]>([]);
  const [newTodo, setNewTodo] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadTodos = useCallback(async () => {
    try {
      const todos = await invoke<TodoItem[]>("todo_list");
      setItems(todos);
    } catch (e) {
      console.error("Failed to load todos:", e);
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    loadTodos();
  }, [loadTodos]);

  const addTodo = async () => {
    if (!newTodo.trim()) return;
    setLoading(true);
    try {
      await invoke<TodoItem>("todo_add", {
        text: newTodo.trim(),
        dueDate: null,
      });
      setNewTodo("");
      await loadTodos();
    } catch (e) {
      console.error("Failed to add todo:", e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const toggleItem = async (id: string) => {
    try {
      await invoke("todo_toggle", { id });
      await loadTodos();
    } catch (e) {
      console.error("Failed to toggle item:", e);
      setError(String(e));
    }
  };

  const removeItem = async (id: string) => {
    try {
      await invoke("todo_remove", { id });
      await loadTodos();
    } catch (e) {
      console.error("Failed to remove item:", e);
      setError(String(e));
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      addTodo();
    }
  };

  const pending = items.filter((t) => !t.done);
  const completed = items.filter((t) => t.done);

  return (
    <div style={styles.container}>
      <div style={styles.headerRow}>
        <div style={styles.inputRow}>
          <input
            type="text"
            value={newTodo}
            onChange={(e) => setNewTodo(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="新しいTODO..."
            style={styles.input}
            disabled={loading}
          />
          <button
            onClick={addTodo}
            disabled={!newTodo.trim() || loading}
            style={{
              ...styles.addButton,
              opacity: newTodo.trim() ? 1 : 0.4,
            }}
          >
            +
          </button>
        </div>
      </div>

      {error && (
        <div style={styles.errorBar}>
          {error}
          <button
            onClick={() => setError(null)}
            style={styles.errorDismiss}
          >
            ×
          </button>
        </div>
      )}

      <div style={styles.list}>
        {pending.map((item) => (
          <div key={item.id} style={styles.item}>
            <button
              onClick={() => toggleItem(item.id)}
              style={styles.checkbox}
              title="完了にする"
            >
              ○
            </button>
            <span style={styles.text}>{item.text}</span>
            <button
              onClick={() => removeItem(item.id)}
              style={styles.deleteButton}
              title="削除"
            >
              ×
            </button>
          </div>
        ))}

        {completed.length > 0 && (
          <>
            <div style={styles.completedHeader}>
              完了済み ({completed.length})
            </div>
            {completed.map((item) => (
              <div key={item.id} style={{ ...styles.item, opacity: 0.5 }}>
                <button
                  onClick={() => toggleItem(item.id)}
                  style={styles.checkbox}
                  title="未完了に戻す"
                >
                  ✓
                </button>
                <span
                  style={{ ...styles.text, textDecoration: "line-through" }}
                >
                  {item.text}
                </span>
                <button
                  onClick={() => removeItem(item.id)}
                  style={styles.deleteButton}
                  title="削除"
                >
                  ×
                </button>
              </div>
            ))}
          </>
        )}

        {items.length === 0 && (
          <div style={styles.empty}>TODOはありません</div>
        )}
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
  headerRow: {
    borderBottom: "1px solid var(--border)",
  },
  inputRow: {
    display: "flex",
    gap: "4px",
    padding: "8px 12px",
  },
  input: {
    flex: 1,
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "4px",
    color: "var(--text-primary)",
    padding: "4px 8px",
    fontSize: "12px",
    fontFamily: "inherit",
    outline: "none",
  },
  addButton: {
    background: "var(--accent)",
    border: "none",
    color: "white",
    borderRadius: "4px",
    width: "28px",
    height: "28px",
    cursor: "pointer",
    fontSize: "16px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  errorBar: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 12px",
    fontSize: "11px",
    color: "#e55",
    background: "rgba(255,50,50,0.08)",
    borderBottom: "1px solid var(--border)",
  },
  errorDismiss: {
    background: "none",
    border: "none",
    color: "#e55",
    cursor: "pointer",
    fontSize: "14px",
    marginLeft: "auto",
    padding: "0 2px",
  },
  list: {
    flex: 1,
    overflowY: "auto",
    padding: "4px 0",
  },
  item: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 12px",
    fontSize: "13px",
  },
  checkbox: {
    background: "none",
    border: "1px solid var(--text-muted)",
    color: "var(--text-secondary)",
    borderRadius: "50%",
    width: "20px",
    height: "20px",
    cursor: "pointer",
    fontSize: "11px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    flexShrink: 0,
    padding: 0,
  },
  text: {
    flex: 1,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  deleteButton: {
    background: "none",
    border: "none",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "14px",
    padding: "0 2px",
    opacity: 0.5,
  },
  completedHeader: {
    padding: "8px 12px 4px",
    fontSize: "11px",
    color: "var(--text-muted)",
    fontWeight: 600,
  },
  empty: {
    padding: "20px 12px",
    textAlign: "center",
    color: "var(--text-muted)",
    fontSize: "12px",
  },
};
