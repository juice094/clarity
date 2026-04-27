import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import TaskPanel from "./TaskPanel";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "list_tasks") {
      return [
        { id: "t1", name: "Test task", status: "running", priority: "normal", created_at: new Date().toISOString() },
      ];
    }
    if (cmd === "cancel_task") {
      return undefined;
    }
    return undefined;
  }),
}));

describe("TaskPanel smoke test", () => {
  it("renders task list when open", async () => {
    render(<TaskPanel isOpen={true} onClose={vi.fn()} />);
    expect(await screen.findByText("Test task")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    render(<TaskPanel isOpen={false} onClose={vi.fn()} />);
    expect(screen.queryByText("Test task")).not.toBeInTheDocument();
  });

  it("calls onClose when close button clicked", async () => {
    const onClose = vi.fn();
    render(<TaskPanel isOpen={true} onClose={onClose} />);
    const closeBtn = await screen.findByLabelText("Close");
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
