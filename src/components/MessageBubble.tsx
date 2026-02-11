import type { ChatMessage } from "../types";
import { MermaidDiagram } from "./MermaidDiagram";

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

/** A parsed segment of message content */
type ContentSegment =
  | { type: "text"; content: string }
  | { type: "code"; language: string; content: string };

/** Parse message content into text and code block segments */
function parseContent(content: string): ContentSegment[] {
  const segments: ContentSegment[] = [];
  const codeBlockRegex = /```(\w*)\n([\s\S]*?)```/g;

  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = codeBlockRegex.exec(content)) !== null) {
    // Text before this code block
    if (match.index > lastIndex) {
      segments.push({ type: "text", content: content.slice(lastIndex, match.index) });
    }
    segments.push({
      type: "code",
      language: match[1] || "",
      content: match[2].trimEnd(),
    });
    lastIndex = match.index + match[0].length;
  }

  // Remaining text after last code block
  if (lastIndex < content.length) {
    segments.push({ type: "text", content: content.slice(lastIndex) });
  }

  return segments;
}

function renderContent(content: string) {
  const segments = parseContent(content);

  return segments.map((segment, idx) => {
    if (segment.type === "code") {
      if (segment.language === "mermaid") {
        return <MermaidDiagram key={idx} code={segment.content} />;
      }
      return (
        <pre key={idx} style={codeStyles.block}>
          {segment.language && (
            <div style={codeStyles.lang}>{segment.language}</div>
          )}
          <code>{segment.content}</code>
        </pre>
      );
    }

    // Render text lines
    return (
      <div key={idx}>
        {renderTextLines(segment.content)}
      </div>
    );
  });
}

function renderTextLines(text: string) {
  const lines = text.split("\n");

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

    // Numbered list items
    const numberedMatch = line.match(/^(\d+)\.\s+(.+)/);
    if (numberedMatch) {
      return (
        <div key={i} style={{ paddingLeft: "20px", position: "relative" }}>
          <span style={{ position: "absolute", left: "0", color: "var(--text-muted)" }}>
            {numberedMatch[1]}.
          </span>
          {renderInline(numberedMatch[2])}
        </div>
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

const codeStyles: Record<string, React.CSSProperties> = {
  block: {
    margin: "8px 0",
    padding: "10px 12px",
    background: "rgba(0, 0, 0, 0.3)",
    borderRadius: "6px",
    fontSize: "12px",
    fontFamily: "monospace",
    lineHeight: "1.5",
    overflowX: "auto",
    whiteSpace: "pre-wrap",
  },
  lang: {
    fontSize: "10px",
    color: "var(--text-muted)",
    marginBottom: "4px",
    textTransform: "uppercase" as const,
    letterSpacing: "0.5px",
  },
};
