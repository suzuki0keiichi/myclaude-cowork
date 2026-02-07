interface ApprovalDialogProps {
  description: string;
  details: string[];
  onApprove: () => void;
  onReject: () => void;
}

export function ApprovalDialog({
  description,
  details,
  onApprove,
  onReject,
}: ApprovalDialogProps) {
  return (
    <div style={styles.overlay}>
      <div style={styles.dialog}>
        <div style={styles.header}>確認してください</div>

        <div style={styles.body}>
          <p style={styles.description}>{description}</p>

          {details.length > 0 && (
            <div style={styles.detailList}>
              {details.map((detail, i) => (
                <div key={i} style={styles.detailItem}>
                  {detail}
                </div>
              ))}
            </div>
          )}
        </div>

        <div style={styles.actions}>
          <button onClick={onReject} style={styles.rejectButton}>
            やり直す
          </button>
          <button onClick={onApprove} style={styles.approveButton}>
            OK!
          </button>
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  overlay: {
    position: "fixed",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    background: "rgba(0,0,0,0.6)",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    zIndex: 1000,
  },
  dialog: {
    background: "var(--bg-secondary)",
    borderRadius: "12px",
    border: "1px solid var(--border)",
    maxWidth: "440px",
    width: "90%",
    overflow: "hidden",
  },
  header: {
    padding: "16px 20px",
    fontSize: "15px",
    fontWeight: 700,
    borderBottom: "1px solid var(--border)",
  },
  body: {
    padding: "16px 20px",
  },
  description: {
    fontSize: "14px",
    lineHeight: "1.6",
    marginBottom: "12px",
  },
  detailList: {
    background: "var(--bg-input)",
    borderRadius: "8px",
    padding: "10px 14px",
  },
  detailItem: {
    fontSize: "13px",
    padding: "4px 0",
    color: "var(--text-secondary)",
    borderBottom: "1px solid var(--border)",
  },
  actions: {
    display: "flex",
    gap: "8px",
    padding: "12px 20px 16px",
    justifyContent: "flex-end",
  },
  rejectButton: {
    padding: "8px 20px",
    background: "var(--bg-input)",
    border: "1px solid var(--border)",
    borderRadius: "8px",
    color: "var(--text-secondary)",
    fontSize: "13px",
    fontWeight: 600,
    cursor: "pointer",
    fontFamily: "inherit",
  },
  approveButton: {
    padding: "8px 24px",
    background: "var(--accent)",
    border: "none",
    borderRadius: "8px",
    color: "white",
    fontSize: "13px",
    fontWeight: 600,
    cursor: "pointer",
  },
};
