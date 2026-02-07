import type { ChatMessage } from "../types";

interface MessageBubbleProps {
  message: ChatMessage;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === "user";

  return (
    <div
      style={{
        ...styles.wrapper,
        justifyContent: isUser ? "flex-end" : "flex-start",
      }}
    >
      <div
        style={{
          ...styles.bubble,
          background: isUser ? "var(--bg-user-msg)" : "var(--bg-assistant-msg)",
          borderRadius: isUser
            ? "16px 16px 4px 16px"
            : "16px 16px 16px 4px",
          maxWidth: isUser ? "70%" : "85%",
        }}
      >
        {!isUser && (
          <div style={styles.roleLabel}>Claude</div>
        )}
        <div style={styles.content}>
          {renderContent(message.content)}
        </div>
        <div style={styles.timestamp}>
          {formatTime(message.timestamp)}
        </div>
      </div>
    </div>
  );
}

function renderContent(content: string) {
  // Simple markdown-like rendering
  const lines = content.split("\n");

  return lines.map((line, i) => {
    // Headers
    if (line.startsWith("### ")) {
      return (
        <h4 key={i} style={{ margin: "8px 0 4px", fontSize: "14px", fontWeight: 600 }}>
          {line.slice(4)}
        </h4>
      );
    }
    if (line.startsWith("## ")) {
      return (
        <h3 key={i} style={{ margin: "10px 0 4px", fontSize: "15px", fontWeight: 600 }}>
          {line.slice(3)}
        </h3>
      );
    }
    if (line.startsWith("# ")) {
      return (
        <h2 key={i} style={{ margin: "12px 0 6px", fontSize: "16px", fontWeight: 700 }}>
          {line.slice(2)}
        </h2>
      );
    }

    // List items
    if (line.startsWith("- ") || line.startsWith("* ")) {
      return (
        <div key={i} style={{ paddingLeft: "16px", position: "relative" }}>
          <span style={{ position: "absolute", left: "4px" }}>•</span>
          {renderInline(line.slice(2))}
        </div>
      );
    }

    // Code blocks (simple)
    if (line.startsWith("```")) {
      return null; // Simplified for now
    }

    // Inline code
    if (line.includes("`")) {
      return <div key={i}>{renderInline(line)}</div>;
    }

    // Empty line
    if (line.trim() === "") {
      return <div key={i} style={{ height: "8px" }} />;
    }

    return <div key={i}>{renderInline(line)}</div>;
  });
}

function renderInline(text: string) {
  // Handle inline code
  const parts = text.split(/(`[^`]+`)/g);
  return parts.map((part, i) => {
    if (part.startsWith("`") && part.endsWith("`")) {
      return (
        <code
          key={i}
          style={{
            background: "rgba(255,255,255,0.1)",
            padding: "1px 5px",
            borderRadius: "3px",
            fontSize: "13px",
            fontFamily: "monospace",
          }}
        >
          {part.slice(1, -1)}
        </code>
      );
    }
    // Handle bold
    const boldParts = part.split(/(\*\*[^*]+\*\*)/g);
    return boldParts.map((bp, j) => {
      if (bp.startsWith("**") && bp.endsWith("**")) {
        return <strong key={`${i}-${j}`}>{bp.slice(2, -2)}</strong>;
      }
      return <span key={`${i}-${j}`}>{bp}</span>;
    });
  });
}

function formatTime(timestamp: string): string {
  try {
    const date = new Date(timestamp);
    return date.toLocaleTimeString("ja-JP", {
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return "";
  }
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    display: "flex",
    padding: "4px 16px",
  },
  bubble: {
    padding: "10px 14px",
    wordBreak: "break-word" as const,
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
  },
  timestamp: {
    fontSize: "10px",
    color: "var(--text-muted)",
    marginTop: "4px",
    textAlign: "right" as const,
  },
};
