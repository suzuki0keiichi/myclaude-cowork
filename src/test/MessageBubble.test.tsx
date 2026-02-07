import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MessageBubble } from "../components/MessageBubble";

describe("MessageBubble", () => {
  it("renders user message", () => {
    render(
      <MessageBubble
        message={{
          id: "1",
          role: "user",
          content: "テストメッセージ",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    expect(screen.getByText("テストメッセージ")).toBeInTheDocument();
  });

  it("renders assistant message with Claude label", () => {
    render(
      <MessageBubble
        message={{
          id: "2",
          role: "assistant",
          content: "こんにちは",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    expect(screen.getByText("Claude")).toBeInTheDocument();
    expect(screen.getByText("こんにちは")).toBeInTheDocument();
  });

  it("does not show Claude label for user messages", () => {
    render(
      <MessageBubble
        message={{
          id: "3",
          role: "user",
          content: "ユーザーの発言",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    expect(screen.queryByText("Claude")).not.toBeInTheDocument();
  });

  it("renders bold text with **markers**", () => {
    render(
      <MessageBubble
        message={{
          id: "4",
          role: "assistant",
          content: "これは**太字**です",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    const bold = screen.getByText("太字");
    expect(bold.tagName).toBe("STRONG");
  });

  it("renders inline code with backticks", () => {
    render(
      <MessageBubble
        message={{
          id: "5",
          role: "assistant",
          content: "コマンドは `ls -la` です",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    const code = screen.getByText("ls -la");
    expect(code.tagName).toBe("CODE");
  });

  it("renders list items with bullet prefix", () => {
    render(
      <MessageBubble
        message={{
          id: "6",
          role: "assistant",
          content: "- 項目1\n- 項目2",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    expect(screen.getByText("項目1")).toBeInTheDocument();
    expect(screen.getByText("項目2")).toBeInTheDocument();
  });

  it("renders headers", () => {
    render(
      <MessageBubble
        message={{
          id: "7",
          role: "assistant",
          content: "## セクション\n本文",
          timestamp: new Date().toISOString(),
        }}
      />
    );
    const heading = screen.getByText("セクション");
    expect(heading.tagName).toBe("H3");
  });

  it("displays timestamp", () => {
    const date = new Date(2026, 1, 7, 14, 30);
    render(
      <MessageBubble
        message={{
          id: "8",
          role: "user",
          content: "test",
          timestamp: date.toISOString(),
        }}
      />
    );
    expect(screen.getByText("14:30")).toBeInTheDocument();
  });
});
