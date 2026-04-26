import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import FileBrowser from "./FileBrowser";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string, _args?: unknown) => {
    if (cmd === "get_file_tree") {
      return {
        name: "root",
        type: "directory",
        path: "/",
        children: [
          { name: "src", type: "directory", path: "/src", children: [] },
          { name: "README.md", type: "file", path: "/README.md", size: 1024 },
        ],
      };
    }
    return undefined;
  }),
}));

describe("FileBrowser smoke test", () => {
  it("renders file tree when open", async () => {
    render(<FileBrowser isOpen={true} onClose={vi.fn()} onFileSelect={vi.fn()} />);
    expect(await screen.findByText("root")).toBeInTheDocument();
    // Click to expand root directory
    fireEvent.click(screen.getByText("root"));
    expect(screen.getByText("src")).toBeInTheDocument();
    expect(screen.getByText("README.md")).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    render(<FileBrowser isOpen={false} onClose={vi.fn()} onFileSelect={vi.fn()} />);
    expect(screen.queryByText("src")).not.toBeInTheDocument();
  });

  it("calls onClose when close button clicked", async () => {
    const onClose = vi.fn();
    render(<FileBrowser isOpen={true} onClose={onClose} onFileSelect={vi.fn()} />);
    const closeBtn = await screen.findByLabelText("Close file browser");
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
