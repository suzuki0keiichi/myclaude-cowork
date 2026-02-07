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
  const [todos, setTodos] = useState<TodoItem[]>([]);
  const [newTodo, setNewTodo] = useState("");
  const [loading, setLoading] = useState(false);

  const loadTodos = useCallback(async () => {
    try {
      const items = await invoke<TodoItem[]>("list_todos");
      setTodos(items);
    } catch (e) {
      console.error("Failed to load todos:", e);
    }
  }, []);

  useEffect(() => {
    loadTodos();
  }, [loadTodos]);

  const addTodo = async () => {
    if (!newTodo.trim()) return;
    setLoading(true);
    try {
      await invoke("add_todo", { text: newTodo.trim(), dueDate: null });
      setNewTodo("");
      await loadTodos();
    } catch (e) {
      console.error("Failed to add todo:", e);
    } finally {
      setLoading(false);
    }
  };

  const toggleTodo = async (id: string) => {
    try {
      await invoke("toggle_todo", { id });
      await loadTodos();
    } catch (e) {
      console.error("Failed to toggle todo:", e);
    }
  };

  const removeTodo = async (id: string) => {
    try {
      await invoke("remove_todo", { id });
      await loadTodos();
    } catch (e) {
      console.error("Failed to remove todo:", e);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      addTodo();
    }
  };

  const pending = todos.filter((t) => !t.done);
  const completed = todos.filter((t) => t.done);

  return (
    <div style={styles.container}>
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

      <div style={styles.list}>
        {pending.map((todo) => (
          <div key={todo.id} style={styles.item}>
            <button
              onClick={() => toggleTodo(todo.id)}
              style={styles.checkbox}
              title="完了にする"
            >
              ○
            </button>
            <span style={styles.text}>{todo.text}</span>
            <button
              onClick={() => removeTodo(todo.id)}
              style={styles.deleteButton}
              title="削除"
            >
              ×
            </button>
          </div>
        ))}

        {completed.length > 0 && (
          <>
            <div style={styles.completedHeader}>完了済み ({completed.length})</div>
            {completed.map((todo) => (
              <div key={todo.id} style={{ ...styles.item, opacity: 0.5 }}>
                <button
                  onClick={() => toggleTodo(todo.id)}
                  style={styles.checkbox}
                  title="未完了に戻す"
                >
                  ✓
                </button>
                <span style={{ ...styles.text, textDecoration: "line-through" }}>
                  {todo.text}
                </span>
                <button
                  onClick={() => removeTodo(todo.id)}
                  style={styles.deleteButton}
                  title="削除"
                >
                  ×
                </button>
              </div>
            ))}
          </>
        )}

        {todos.length === 0 && (
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
  inputRow: {
    display: "flex",
    gap: "4px",
    padding: "8px 12px",
    borderBottom: "1px solid var(--border)",
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
