import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import SettingsPanel from "./SettingsPanel";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "get_settings") {
      return {
        model: "gpt-4o",
        provider: "openai",
        approval_mode: "interactive",
        theme: "dark",
        language: "en",
      };
    }
    if (cmd === "get_available_models") {
      return [["openai", "OpenAI", ["gpt-4o"]]];
    }
    if (cmd === "get_approval_modes") {
      return [["interactive", "Interactive"], ["yolo", "Yolo"]];
    }
    if (cmd === "get_local_models") {
      return [];
    }
    return undefined;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { changeLanguage: vi.fn() },
  }),
}));

describe("SettingsPanel smoke test", () => {
  it("renders settings form when open", async () => {
    render(<SettingsPanel isOpen={true} onClose={vi.fn()} />);
    expect(await screen.findByText("settings.title")).toBeInTheDocument();

    // Labels are properly associated with controls via htmlFor
    expect(screen.getByLabelText("settings.provider")).toBeInTheDocument();
    expect(screen.getByLabelText("settings.model")).toBeInTheDocument();
    expect(screen.getByLabelText("settings.language")).toBeInTheDocument();

    // Save button present
    expect(screen.getByText("settings.save")).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    render(<SettingsPanel isOpen={false} onClose={vi.fn()} />);
    expect(screen.queryByText("settings.title")).not.toBeInTheDocument();
  });
});
