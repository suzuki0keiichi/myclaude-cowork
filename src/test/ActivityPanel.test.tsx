import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ActivityPanel } from "../components/ActivityPanel";
import type { ActivityItem } from "../types";

describe("ActivityPanel", () => {
  it("shows empty state when no activities", () => {
    render(<ActivityPanel activities={[]} />);
    expect(screen.getByText("まだ何もしていません")).toBeInTheDocument();
    expect(screen.getByText("アクティビティ")).toBeInTheDocument();
  });

  it("renders activity items", () => {
    const activities: ActivityItem[] = [
      {
        id: "1",
        description: "📄 「report.txt」を読んでいます",
        raw_command: 'Read({"file_path":"/tmp/report.txt"})',
        status: "running",
        timestamp: new Date().toISOString(),
      },
    ];
    render(<ActivityPanel activities={activities} />);
    expect(screen.getByText("📄 「report.txt」を読んでいます")).toBeInTheDocument();
  });

  it("renders multiple activities", () => {
    const activities: ActivityItem[] = [
      {
        id: "1",
        description: "📄 「a.txt」を読んでいます",
        raw_command: null,
        status: "done",
        timestamp: new Date().toISOString(),
      },
      {
        id: "2",
        description: "📁 フォルダ「output」を作成しています",
        raw_command: null,
        status: "running",
        timestamp: new Date().toISOString(),
      },
    ];
    render(<ActivityPanel activities={activities} />);
    expect(screen.getByText("📄 「a.txt」を読んでいます")).toBeInTheDocument();
    expect(screen.getByText("📁 フォルダ「output」を作成しています")).toBeInTheDocument();
  });

  it("shows detail toggle for items with raw_command", () => {
    const activities: ActivityItem[] = [
      {
        id: "1",
        description: "コマンドを実行しています",
        raw_command: 'Bash({"command":"ls -la"})',
        status: "done",
        timestamp: new Date().toISOString(),
      },
    ];
    render(<ActivityPanel activities={activities} />);
    expect(screen.getByText("詳細")).toBeInTheDocument();
  });

  it("toggles raw command visibility", () => {
    const activities: ActivityItem[] = [
      {
        id: "1",
        description: "コマンドを実行しています",
        raw_command: 'Bash({"command":"ls -la"})',
        status: "done",
        timestamp: new Date().toISOString(),
      },
    ];
    render(<ActivityPanel activities={activities} />);

    // Initially raw command is hidden
    expect(screen.queryByText('Bash({"command":"ls -la"})')).not.toBeInTheDocument();

    // Click to show
    fireEvent.click(screen.getByText("詳細"));
    expect(screen.getByText('Bash({"command":"ls -la"})')).toBeInTheDocument();
    expect(screen.getByText("隠す")).toBeInTheDocument();

    // Click to hide again
    fireEvent.click(screen.getByText("隠す"));
    expect(screen.queryByText('Bash({"command":"ls -la"})')).not.toBeInTheDocument();
  });

  it("does not show detail toggle when no raw_command", () => {
    const activities: ActivityItem[] = [
      {
        id: "1",
        description: "完了",
        raw_command: null,
        status: "done",
        timestamp: new Date().toISOString(),
      },
    ];
    render(<ActivityPanel activities={activities} />);
    expect(screen.queryByText("詳細")).not.toBeInTheDocument();
  });
});
