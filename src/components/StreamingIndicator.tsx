interface StreamingIndicatorProps {
  text: string;
}

export function StreamingIndicator({ text }: StreamingIndicatorProps) {
  if (!text) return null;

  return (
    <div style={styles.wrapper}>
      <div style={styles.bubble}>
        <div style={styles.roleLabel}>Claude</div>
        <div style={styles.content}>{text}</div>
        <div style={styles.cursor} />
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    display: "flex",
    justifyContent: "flex-start",
    padding: "4px 16px",
  },
  bubble: {
    background: "var(--bg-assistant-msg)",
    borderRadius: "16px 16px 16px 4px",
    padding: "10px 14px",
    maxWidth: "85%",
  },
  roleLabel: {
    fontSize: "11px",
    fontWeight: 600,
    color: "var(--accent)",
    marginBottom: "4px",
    textTransform: "uppercase" as const,
    letterSpacing: "0.5px",
  },
  content: {
    fontSize: "14px",
    lineHeight: "1.6",
    whiteSpace: "pre-wrap" as const,
  },
  cursor: {
    display: "inline-block",
    width: "8px",
    height: "16px",
    background: "var(--accent)",
    marginLeft: "2px",
    animation: "blink 1s infinite",
    verticalAlign: "text-bottom",
  },
};
