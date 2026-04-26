import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ToolCallIndicator, { type ToolCallInfo } from "./ToolCallIndicator";

const mockCalls: ToolCallInfo[] = [
  { id: "c1", name: "file_read", arguments: { path: "/tmp/test.txt" }, status: "running" },
  { id: "c2", name: "shell", arguments: { command: "echo hello" }, status: "done", result: "hello" },
];

describe("ToolCallIndicator smoke test", () => {
  it("renders tool call cards", () => {
    render(<ToolCallIndicator toolCalls={mockCalls} />);
    expect(screen.getByText("file_read")).toBeInTheDocument();
    expect(screen.getByText("shell")).toBeInTheDocument();
  });

  it("expands card on click", () => {
    render(<ToolCallIndicator toolCalls={mockCalls} />);
    const header = screen.getByText("file_read").closest(".tool-call-header");
    expect(header).toBeTruthy();
    if (header) fireEvent.click(header);
    expect(screen.getByText(/Arguments/)).toBeInTheDocument();
    expect(screen.getByText(/\"path\"/)).toBeInTheDocument();
  });
});
