import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

interface GuiSettings {
  model: string;
  provider: string;
  approval_mode: string;
  theme: string;
}

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

function SettingsPanel({ isOpen, onClose }: SettingsPanelProps) {
  const [settings, setSettings] = useState<GuiSettings>({
    model: "gpt-4o",
    provider: "openai",
    approval_mode: "interactive",
    theme: "dark",
  });
  const [savedSettings, setSavedSettings] = useState<GuiSettings>({
    model: "gpt-4o",
    provider: "openai",
    approval_mode: "interactive",
    theme: "dark",
  });
  const [models, setModels] = useState<[string, string, string[]][]>([]);
  const [approvalModes, setApprovalModes] = useState<[string, string][]>([]);
  const [toast, setToast] = useState("");

  const fetchSettings = useCallback(async () => {
    try {
      const data = await invoke<GuiSettings>("get_settings");
      setSettings(data);
      setSavedSettings(data);
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }, []);

  const fetchMeta = useCallback(async () => {
    try {
      const m = await invoke<[string, string, string[]][]>("get_available_models");
      setModels(m);
      const a = await invoke<[string, string][]>("get_approval_modes");
      setApprovalModes(a);
    } catch (e) {
      console.error("Failed to load settings meta:", e);
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    fetchSettings();
    fetchMeta();
  }, [isOpen, fetchSettings, fetchMeta]);

  const availableModels =
    models.find(([key]) => key === settings.provider)?.[2] ?? [];

  function handleProviderChange(provider: string) {
    const newModels = models.find(([key]) => key === provider)?.[2] ?? [];
    setSettings((prev) => ({
      ...prev,
      provider,
      model: newModels[0] ?? prev.model,
    }));
  }

  async function handleSave() {
    try {
      await invoke("save_settings", { settings });
      setSavedSettings(settings);
      setToast("Settings saved");
      setTimeout(() => setToast(""), 2000);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  function handleReset() {
    setSettings(savedSettings);
  }

  if (!isOpen) return null;

  return (
    <div className="settings-panel">
      <div className="settings-panel-header">
        <h2>Settings</h2>
        <button
          className="settings-panel-close"
          onClick={onClose}
          aria-label="Close"
        >
          ✕
        </button>
      </div>

      <div className="settings-panel-body">
        {/* Model Group */}
        <div className="settings-group">
          <h3>Model</h3>
          <label className="settings-label">Provider</label>
          <select
            className="settings-select"
            value={settings.provider}
            onChange={(e) => handleProviderChange(e.target.value)}
          >
            {models.map(([key, display]) => (
              <option key={key} value={key}>
                {display}
              </option>
            ))}
          </select>

          <label className="settings-label">Model</label>
          <select
            className="settings-select"
            value={settings.model}
            onChange={(e) =>
              setSettings((prev) => ({ ...prev, model: e.target.value }))
            }
          >
            {availableModels.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </div>

        {/* Approval Group */}
        <div className="settings-group">
          <h3>Approval</h3>
          {approvalModes.map(([key, display]) => (
            <label key={key} className="settings-radio-label">
              <input
                type="radio"
                name="approval_mode"
                value={key}
                checked={settings.approval_mode === key}
                onChange={(e) =>
                  setSettings((prev) => ({
                    ...prev,
                    approval_mode: e.target.value,
                  }))
                }
              />
              <span>{display}</span>
            </label>
          ))}
        </div>

        {/* Theme Group */}
        <div className="settings-group">
          <h3>Theme</h3>
          {[
            ["dark", "Dark"],
            ["light", "Light"],
            ["auto", "Auto"],
          ].map(([key, display]) => (
            <label key={key} className="settings-radio-label">
              <input
                type="radio"
                name="theme"
                value={key}
                checked={settings.theme === key}
                onChange={(e) =>
                  setSettings((prev) => ({
                    ...prev,
                    theme: e.target.value,
                  }))
                }
              />
              <span>{display}</span>
            </label>
          ))}
        </div>
      </div>

      <div className="settings-panel-footer">
        <button className="settings-btn settings-btn-primary" onClick={handleSave}>
          Save
        </button>
        <button className="settings-btn settings-btn-secondary" onClick={handleReset}>
          Reset
        </button>
        <button className="settings-btn settings-btn-secondary" onClick={onClose}>
          Cancel
        </button>
        {toast && <span className="settings-toast">{toast}</span>}
      </div>
    </div>
  );
}

export default SettingsPanel;
