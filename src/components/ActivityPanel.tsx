import { useState } from "react";
import type { ActivityItem } from "../types";

interface ActivityPanelProps {
  activities: ActivityItem[];
}

function getStatusDotStyle(status: string): React.CSSProperties {
  return {
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    marginTop: "6px",
    flexShrink: 0,
    background:
      status === "running"
        ? "var(--warning)"
        : status === "done"
        ? "var(--success)"
        : "var(--danger)",
    boxShadow:
      status === "running" ? "0 0 6px var(--warning)" : "none",
  };
}

export function ActivityPanel({ activities }: ActivityPanelProps) {
  const [showRaw, setShowRaw] = useState<string | null>(null);

  if (activities.length === 0) {
    return (
      <div style={styles.container}>
        <div style={styles.header}>
          <span style={styles.headerIcon}>⚡</span>
          アクティビティ
        </div>
        <div style={styles.empty}>
          まだ何もしていません
        </div>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <span style={styles.headerIcon}>⚡</span>
        アクティビティ
      </div>
      <div style={styles.list}>
        {activities.map((activity) => (
          <div key={activity.id} style={styles.item}>
            <div style={styles.itemHeader}>
              <span style={getStatusDotStyle(activity.status)} />
              <span style={styles.description}>
                {activity.description}
              </span>
            </div>
            {activity.raw_command && (
              <div style={styles.rawToggle}>
                <button
                  onClick={() =>
                    setShowRaw(showRaw === activity.id ? null : activity.id)
                  }
                  style={styles.toggleButton}
                >
                  {showRaw === activity.id ? "隠す" : "詳細"}
                </button>
                {showRaw === activity.id && (
                  <div style={styles.rawCommand}>{activity.raw_command}</div>
                )}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    height: "100%",
    display: "flex",
    flexDirection: "column",
    background: "var(--bg-secondary)",
    borderLeft: "1px solid var(--border)",
    overflow: "hidden",
  },
  header: {
    padding: "14px 16px",
    fontSize: "13px",
    fontWeight: 600,
    color: "var(--text-secondary)",
    borderBottom: "1px solid var(--border)",
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  headerIcon: {
    fontSize: "14px",
  },
  empty: {
    padding: "20px 16px",
    color: "var(--text-muted)",
    fontSize: "13px",
    textAlign: "center",
  },
  list: {
    flex: 1,
    overflowY: "auto",
    padding: "8px 0",
  },
  item: {
    padding: "8px 16px",
    borderBottom: "1px solid var(--border)",
  },
  itemHeader: {
    display: "flex",
    alignItems: "flex-start",
    gap: "8px",
  },
  description: {
    fontSize: "13px",
    color: "var(--text-primary)",
    lineHeight: "1.4",
  },
  rawToggle: {
    marginTop: "4px",
    marginLeft: "16px",
  },
  toggleButton: {
    background: "none",
    border: "none",
    color: "var(--text-muted)",
    fontSize: "11px",
    cursor: "pointer",
    padding: "2px 0",
    textDecoration: "underline",
  },
  rawCommand: {
    marginTop: "4px",
    padding: "6px 8px",
    background: "rgba(0,0,0,0.3)",
    borderRadius: "4px",
    fontSize: "11px",
    fontFamily: "monospace",
    color: "var(--text-muted)",
    wordBreak: "break-all",
    maxHeight: "100px",
    overflowY: "auto",
  },
};
