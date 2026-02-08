import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number | null;
  children: FileEntry[] | null;
}

interface FileBrowserProps {
  workingDir: string;
  onFileSelect?: (path: string) => void;
  selectedFiles?: string[];
  onFileToggle?: (path: string) => void;
}

export function FileBrowser({ workingDir, onFileSelect, selectedFiles = [], onFileToggle }: FileBrowserProps) {
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [currentPath, setCurrentPath] = useState(workingDir);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadDirectory = useCallback(async (path: string) => {
    setLoading(true);
    setError(null);
    try {
      const files = await invoke<FileEntry[]>("list_files", { path });
      setEntries(files);
      setCurrentPath(path);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (workingDir) {
      loadDirectory(workingDir);
    }
  }, [workingDir, loadDirectory]);

  const handleClick = (entry: FileEntry) => {
    if (entry.is_dir) {
      loadDirectory(entry.path);
    } else {
      onFileSelect?.(entry.path);
    }
  };

  const goUp = () => {
    const parent = currentPath.replace(/[/\\][^/\\]+$/, "");
    if (parent && parent !== currentPath) {
      loadDirectory(parent);
    }
  };

  return (
    <div style={styles.container}>
      <div style={styles.pathBar}>
        <button onClick={goUp} style={styles.upButton} title="上のフォルダへ">
          ..
        </button>
        <span style={styles.currentPath} title={currentPath}>
          {currentPath.split(/[/\\]/).pop() || currentPath}
        </span>
        <button
          onClick={() => loadDirectory(currentPath)}
          style={styles.refreshButton}
          title="更新"
        >
          ↻
        </button>
      </div>

      {loading && <div style={styles.loading}>読み込み中...</div>}
      {error && <div style={styles.error}>{error}</div>}

      <div style={styles.list}>
        {entries.map((entry) => (
          <div
            key={entry.path}
            style={{
              ...styles.entry,
              ...(selectedFiles.includes(entry.path) ? styles.entrySelected : {}),
            }}
            onClick={() => handleClick(entry)}
            title={entry.path}
          >
            {!entry.is_dir && onFileToggle && (
              <input
                type="checkbox"
                checked={selectedFiles.includes(entry.path)}
                onChange={(e) => {
                  e.stopPropagation();
                  onFileToggle(entry.path);
                }}
                onClick={(e) => e.stopPropagation()}
                style={styles.checkbox}
              />
            )}
            <span style={styles.icon}>
              {entry.is_dir ? "📁" : getFileIcon(entry.name)}
            </span>
            <span style={styles.name}>{entry.name}</span>
            {!entry.is_dir && entry.size !== null && (
              <span style={styles.size}>{formatSize(entry.size)}</span>
            )}
          </div>
        ))}
        {!loading && entries.length === 0 && (
          <div style={styles.empty}>フォルダは空です</div>
        )}
      </div>
    </div>
  );
}

function formatSize(bytes: number | null): string {
  if (bytes === null) return "";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function getFileIcon(name: string): string {
  const ext = name.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "pdf": return "📕";
    case "doc": case "docx": return "📘";
    case "xls": case "xlsx": case "csv": return "📊";
    case "ppt": case "pptx": return "📙";
    case "jpg": case "jpeg": case "png": case "gif": case "svg": return "🖼️";
    case "mp4": case "mov": case "avi": return "🎬";
    case "mp3": case "wav": return "🎵";
    case "zip": case "rar": case "7z": return "📦";
    case "txt": case "md": return "📄";
    case "json": case "xml": case "yaml": case "yml": return "📋";
    case "js": case "ts": case "py": case "rs": case "go": return "💻";
    default: return "📄";
  }
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  pathBar: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "8px 12px",
    borderBottom: "1px solid var(--border)",
    fontSize: "12px",
  },
  upButton: {
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    color: "var(--text-secondary)",
    borderRadius: "4px",
    padding: "2px 8px",
    cursor: "pointer",
    fontSize: "12px",
    fontFamily: "inherit",
  },
  currentPath: {
    flex: 1,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    color: "var(--text-muted)",
    fontSize: "11px",
  },
  refreshButton: {
    background: "none",
    border: "none",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "14px",
    padding: "0 4px",
  },
  list: {
    flex: 1,
    overflowY: "auto",
    padding: "4px 0",
  },
  entry: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 12px",
    cursor: "pointer",
    fontSize: "13px",
    transition: "background 0.1s",
  },
  entrySelected: {
    background: "var(--accent-muted, rgba(59, 130, 246, 0.1))",
  },
  checkbox: {
    flexShrink: 0,
    cursor: "pointer",
    accentColor: "var(--accent)",
  },
  icon: {
    fontSize: "13px",
    flexShrink: 0,
  },
  name: {
    flex: 1,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  size: {
    fontSize: "11px",
    color: "var(--text-muted)",
    flexShrink: 0,
  },
  loading: {
    padding: "12px",
    textAlign: "center",
    color: "var(--text-muted)",
    fontSize: "12px",
  },
  error: {
    padding: "8px 12px",
    color: "var(--danger)",
    fontSize: "12px",
  },
  empty: {
    padding: "20px 12px",
    textAlign: "center",
    color: "var(--text-muted)",
    fontSize: "12px",
  },
};
