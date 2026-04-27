import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import DiffViewer, { type DiffHunk } from "./DiffViewer";

const mockHunks: DiffHunk[] = [
  {
    old_start: 1,
    new_start: 1,
    lines: [
      { tag: "equal", content: "line 1" },
      { tag: "delete", content: "old line 2" },
      { tag: "insert", content: "new line 2" },
    ],
  },
];

describe("DiffViewer smoke test", () => {
  it("renders diff hunks with markers", () => {
    render(<DiffViewer hunks={mockHunks} />);
    expect(screen.getByText("line 1")).toBeInTheDocument();
    expect(screen.getByText("old line 2")).toBeInTheDocument();
    expect(screen.getByText("new line 2")).toBeInTheDocument();
    expect(screen.getByText("@@ -1 +1 @@")).toBeInTheDocument();
  });

  it("renders empty when no hunks", () => {
    render(<DiffViewer hunks={[]} />);
    expect(screen.queryByText("@@")).not.toBeInTheDocument();
  });
});
