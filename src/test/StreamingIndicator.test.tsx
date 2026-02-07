import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { StreamingIndicator } from "../components/StreamingIndicator";

describe("StreamingIndicator", () => {
  it("renders nothing when text is empty", () => {
    const { container } = render(<StreamingIndicator text="" />);
    expect(container.firstChild).toBeNull();
  });

  it("renders streaming text", () => {
    render(<StreamingIndicator text="応答を生成中..." />);
    expect(screen.getByText("応答を生成中...")).toBeInTheDocument();
  });

  it("shows Claude label", () => {
    render(<StreamingIndicator text="テスト" />);
    expect(screen.getByText("Claude")).toBeInTheDocument();
  });
});
