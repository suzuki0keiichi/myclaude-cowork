import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SetupScreen } from "../components/SetupScreen";

describe("SetupScreen", () => {
  it("renders welcome screen", () => {
    render(<SetupScreen onSetup={vi.fn()} />);
    expect(screen.getByText("Cowork")).toBeInTheDocument();
    expect(screen.getByText("Claudeと一緒に作業しましょう")).toBeInTheDocument();
    expect(screen.getByText("作業フォルダを選んでください")).toBeInTheDocument();
  });

  it("calls onSetup with path when clicking button", async () => {
    const onSetup = vi.fn();
    const user = userEvent.setup();
    render(<SetupScreen onSetup={onSetup} />);

    // Manual input is hidden by default; click toggle to show it
    await user.click(screen.getByText("または手動で入力"));

    const input = screen.getByPlaceholderText(/例:/);
    await user.type(input, "C:\\Users\\test\\Documents");
    await user.click(screen.getByText("はじめる"));

    expect(onSetup).toHaveBeenCalledWith("C:\\Users\\test\\Documents");
  });

  it("calls onSetup on Enter key", async () => {
    const onSetup = vi.fn();
    const user = userEvent.setup();
    render(<SetupScreen onSetup={onSetup} />);

    // Manual input is hidden by default; click toggle to show it
    await user.click(screen.getByText("または手動で入力"));

    const input = screen.getByPlaceholderText(/例:/);
    await user.type(input, "/home/user/work{Enter}");

    expect(onSetup).toHaveBeenCalledWith("/home/user/work");
  });

  it("does not call onSetup with empty path", async () => {
    const onSetup = vi.fn();
    const user = userEvent.setup();
    render(<SetupScreen onSetup={onSetup} />);

    await user.click(screen.getByText("はじめる"));
    expect(onSetup).not.toHaveBeenCalled();
  });

  it("button is visually disabled when path is empty", () => {
    render(<SetupScreen onSetup={vi.fn()} />);
    const button = screen.getByText("はじめる");
    // Button should have reduced opacity
    expect(button.style.opacity).toBe("0.5");
  });
});
