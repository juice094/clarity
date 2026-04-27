import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import Sidebar, { type Session, type ToolItem } from "./Sidebar";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: Record<string, unknown>) => {
      if (options && "count" in options) {
        return `${options.count} ${key}`;
      }
      return key;
    },
  }),
}));

const mockSessions: Session[] = [
  { id: "s1", title: "Chat 1", created_at: Date.now(), updated_at: Date.now(), messages: [] },
  { id: "s2", title: "Chat 2", created_at: Date.now(), updated_at: Date.now(), messages: [] },
];

const mockTools: ToolItem[] = [
  { id: "files", label: "Files", icon: <span data-testid="icon-files" />, onClick: vi.fn() },
];

describe("Sidebar smoke test", () => {
  it("renders session list", () => {
    render(
      <Sidebar
        sessions={mockSessions}
        activeSessionId="s1"
        collapsed={false}
        onToggle={vi.fn()}
        onSelect={vi.fn()}
        onNew={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    expect(screen.getByText("Chat 1")).toBeInTheDocument();
    expect(screen.getByText("Chat 2")).toBeInTheDocument();
  });

  it("calls onSelect when session clicked", () => {
    const onSelect = vi.fn();
    render(
      <Sidebar
        sessions={mockSessions}
        activeSessionId="s1"
        collapsed={false}
        onToggle={vi.fn()}
        onSelect={onSelect}
        onNew={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    fireEvent.click(screen.getByText("Chat 2"));
    expect(onSelect).toHaveBeenCalledWith("s2");
  });

  it("calls onNew when new session button clicked", () => {
    const onNew = vi.fn();
    render(
      <Sidebar
        sessions={mockSessions}
        activeSessionId="s1"
        collapsed={false}
        onToggle={vi.fn()}
        onSelect={vi.fn()}
        onNew={onNew}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    const newBtn = screen.getByTitle("sidebar.newSession");
    fireEvent.click(newBtn);
    expect(onNew).toHaveBeenCalledTimes(1);
  });

  it("renders tools when provided", () => {
    render(
      <Sidebar
        sessions={mockSessions}
        activeSessionId="s1"
        collapsed={false}
        onToggle={vi.fn()}
        onSelect={vi.fn()}
        onNew={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
        tools={mockTools}
        activeTool="files"
      />
    );
    expect(screen.getByText("Files")).toBeInTheDocument();
  });
});
