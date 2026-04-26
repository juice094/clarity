import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import App from "./App";

// Mock Tauri core API
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "get_launch_status") {
      return { needs_onboarding: false, configured: true };
    }
    if (cmd === "get_prewarm_status") {
      return null;
    }
    if (cmd === "list_sessions") {
      return [];
    }
    if (cmd === "get_settings") {
      return { theme: "dark" };
    }
    if (cmd === "get_agent_status") {
      return "idle";
    }
    return undefined;
  }),
}));

// Mock Tauri event API
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => {
    return () => {};
  }),
}));

// Mock i18n
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { changeLanguage: () => Promise.resolve() },
  }),
}));

// Mock updater plugin (dynamic import in App)
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: vi.fn(async () => null),
}));

describe("App smoke test", () => {
  it("renders without crashing and shows header", async () => {
    render(<App />);

    // Header title should appear (h1 in header, not sidebar title)
    expect(await screen.findByRole("heading", { name: "Clarity" })).toBeInTheDocument();

    // Chat input placeholder
    expect(screen.getByPlaceholderText("Type a message...")).toBeInTheDocument();

    // Send button
    expect(screen.getByLabelText("Send")).toBeInTheDocument();
  });
});
