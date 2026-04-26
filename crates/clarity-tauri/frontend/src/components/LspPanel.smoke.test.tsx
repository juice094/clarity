import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import LspPanel from "./LspPanel";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "lsp_list") {
      return [
        { id: "p1", server_path: "rust-analyzer", root_path: "/project", status: "running" },
      ];
    }
    return undefined;
  }),
}));

describe("LspPanel smoke test", () => {
  it("renders LSP panel when open", async () => {
    render(<LspPanel isOpen={true} onClose={vi.fn()} />);
    expect(await screen.findByText("LSP Servers")).toBeInTheDocument();
    expect(screen.getByText("rust-analyzer")).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    render(<LspPanel isOpen={false} onClose={vi.fn()} />);
    expect(screen.queryByText("LSP Servers")).not.toBeInTheDocument();
  });

  it("calls onClose when close button clicked", async () => {
    const onClose = vi.fn();
    render(<LspPanel isOpen={true} onClose={onClose} />);
    const closeBtn = await screen.findByLabelText("Close");
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
