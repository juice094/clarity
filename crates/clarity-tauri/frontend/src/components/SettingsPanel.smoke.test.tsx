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
    // Provider and model selects are present (3 comboboxes: provider, model, language)
    const combos = screen.getAllByRole("combobox");
    expect(combos.length).toBeGreaterThanOrEqual(2);
    // Save button present
    expect(screen.getByText("settings.save")).toBeInTheDocument();
  });

  it("renders nothing when closed", () => {
    render(<SettingsPanel isOpen={false} onClose={vi.fn()} />);
    expect(screen.queryByText("settings.title")).not.toBeInTheDocument();
  });
});
