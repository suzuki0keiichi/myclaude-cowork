import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ChatInput } from "../components/ChatInput";

describe("ChatInput", () => {
  it("renders input and send button", () => {
    render(<ChatInput onSend={vi.fn()} disabled={false} />);
    expect(screen.getByPlaceholderText("メッセージを入力...")).toBeInTheDocument();
    expect(screen.getByText("送信")).toBeInTheDocument();
  });

  it("shows disabled placeholder when loading", () => {
    render(<ChatInput onSend={vi.fn()} disabled={true} />);
    expect(screen.getByPlaceholderText("応答を待っています...")).toBeInTheDocument();
  });

  it("calls onSend when clicking send button", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<ChatInput onSend={onSend} disabled={false} />);

    const textarea = screen.getByPlaceholderText("メッセージを入力...");
    await user.type(textarea, "テスト送信");
    await user.click(screen.getByText("送信"));

    expect(onSend).toHaveBeenCalledWith("テスト送信");
  });

  it("calls onSend on Enter key", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<ChatInput onSend={onSend} disabled={false} />);

    const textarea = screen.getByPlaceholderText("メッセージを入力...");
    await user.type(textarea, "エンターで送信{Enter}");

    expect(onSend).toHaveBeenCalledWith("エンターで送信");
  });

  it("does not send on Shift+Enter", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<ChatInput onSend={onSend} disabled={false} />);

    const textarea = screen.getByPlaceholderText("メッセージを入力...");
    await user.type(textarea, "改行{Shift>}{Enter}{/Shift}続き");

    expect(onSend).not.toHaveBeenCalled();
  });

  it("does not send empty message", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<ChatInput onSend={onSend} disabled={false} />);

    await user.click(screen.getByText("送信"));

    expect(onSend).not.toHaveBeenCalled();
  });

  it("does not send when disabled", () => {
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} disabled={true} />);

    screen.getByPlaceholderText("応答を待っています...");
    const button = screen.getByText("送信");
    expect(button).toBeDisabled();
  });

  it("clears input after sending", async () => {
    const onSend = vi.fn();
    const user = userEvent.setup();
    render(<ChatInput onSend={onSend} disabled={false} />);

    const textarea = screen.getByPlaceholderText("メッセージを入力...") as HTMLTextAreaElement;
    await user.type(textarea, "メッセージ{Enter}");

    expect(textarea.value).toBe("");
  });
});
