import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ComputerUsePanel from "./ComputerUsePanel";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "computer_check_bridge") {
      return true;
    }
    return undefined;
  }),
}));

describe("ComputerUsePanel smoke test", () => {
  it("renders panel when open", () => {
    render(<ComputerUsePanel isOpen={true} onClose={vi.fn()} />);
    expect(screen.getByText("Computer Use")).toBeInTheDocument();
    expect(screen.getByText(/Screenshot/)).toBeInTheDocument();
    expect(screen.getByText(/Check Env/)).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    render(<ComputerUsePanel isOpen={false} onClose={vi.fn()} />);
    expect(screen.queryByText("Computer Use")).not.toBeInTheDocument();
  });

  it("calls onClose when close button clicked", () => {
    const onClose = vi.fn();
    render(<ComputerUsePanel isOpen={true} onClose={onClose} />);
    const closeBtn = screen.getByLabelText("Close");
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
