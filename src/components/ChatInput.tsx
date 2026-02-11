import { useState, useRef, useEffect } from "react";

interface ChatInputProps {
  onSend: (message: string) => void;
  disabled: boolean;
  isLoading?: boolean;
  onCancel?: () => void;
}

export function ChatInput({ onSend, disabled, isLoading, onCancel }: ChatInputProps) {
  const [input, setInput] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const composingRef = useRef(false);

  useEffect(() => {
    if (!disabled && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [disabled]);

  const handleSubmit = () => {
    if (input.trim() && !disabled) {
      onSend(input.trim());
      setInput("");
      if (textareaRef.current) {
        textareaRef.current.style.height = "auto";
      }
    }
  };

  const handleCompositionStart = () => {
    composingRef.current = true;
  };

  const handleCompositionEnd = () => {
    // Delay to ensure the final Enter keydown fires while still "composing"
    setTimeout(() => {
      composingRef.current = false;
    }, 50);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey && !composingRef.current && !e.nativeEvent.isComposing) {
      e.preventDefault();
      handleSubmit();
    }
  };

  const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    // Auto-resize
    const textarea = e.target;
    textarea.style.height = "auto";
    textarea.style.height = Math.min(textarea.scrollHeight, 150) + "px";
  };

  return (
    <div style={styles.container}>
      <div style={styles.inputWrapper}>
        <textarea
          ref={textareaRef}
          value={input}
          onChange={handleInput}
          onKeyDown={handleKeyDown}
          onCompositionStart={handleCompositionStart}
          onCompositionEnd={handleCompositionEnd}
          placeholder={disabled ? "応答を待っています..." : "メッセージを入力..."}
          disabled={disabled}
          rows={1}
          style={{
            ...styles.textarea,
            opacity: disabled ? 0.6 : 1,
          }}
        />
        {isLoading && onCancel ? (
          <button
            onClick={onCancel}
            style={styles.cancelButton}
          >
            中止
          </button>
        ) : (
          <button
            onClick={handleSubmit}
            disabled={disabled || !input.trim()}
            style={{
              ...styles.sendButton,
              opacity: disabled || !input.trim() ? 0.4 : 1,
            }}
          >
            送信
          </button>
        )}
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    padding: "12px 16px",
    borderTop: "1px solid var(--border)",
    background: "var(--bg-secondary)",
  },
  inputWrapper: {
    display: "flex",
    alignItems: "flex-end",
    gap: "8px",
    background: "var(--bg-input)",
    borderRadius: "12px",
    padding: "8px 12px",
    border: "1px solid var(--border)",
  },
  textarea: {
    flex: 1,
    background: "transparent",
    border: "none",
    outline: "none",
    color: "var(--text-primary)",
    fontSize: "14px",
    fontFamily: "inherit",
    lineHeight: "1.5",
    resize: "none" as const,
    maxHeight: "150px",
  },
  sendButton: {
    background: "var(--accent)",
    color: "white",
    border: "none",
    borderRadius: "8px",
    padding: "6px 16px",
    fontSize: "13px",
    fontWeight: 600,
    cursor: "pointer",
    whiteSpace: "nowrap" as const,
    flexShrink: 0,
  },
  cancelButton: {
    background: "#dc3545",
    color: "white",
    border: "none",
    borderRadius: "8px",
    padding: "6px 16px",
    fontSize: "13px",
    fontWeight: 600,
    cursor: "pointer",
    whiteSpace: "nowrap" as const,
    flexShrink: 0,
  },
};
