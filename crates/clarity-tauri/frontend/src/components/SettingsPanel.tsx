import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";

export interface GuiSettings {
  model: string;
  provider: string;
  approval_mode: string;
  theme: string;
  local_model_path?: string;
  network_probe_url?: string;
  language?: string;
}

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

function SettingsPanel({ isOpen, onClose }: SettingsPanelProps) {
  const { t, i18n } = useTranslation();
  const [settings, setSettings] = useState<GuiSettings>({
    model: "gpt-4o",
    provider: "openai",
    approval_mode: "interactive",
    theme: "dark",
    language: "zh",
  });
  const [savedSettings, setSavedSettings] = useState<GuiSettings>({
    model: "gpt-4o",
    provider: "openai",
    approval_mode: "interactive",
    theme: "dark",
    language: "zh",
  });
  const [models, setModels] = useState<[string, string, string[]][]>([]);
  const [approvalModes, setApprovalModes] = useState<[string, string][]>([]);
  const [localModels, setLocalModels] = useState<[string, string][]>([]);
  const [toast, setToast] = useState("");

  const fetchSettings = useCallback(async () => {
    try {
      const data = await invoke<GuiSettings>("get_settings");
      setSettings(data);
      setSavedSettings(data);
      if (data.language) {
        i18n.changeLanguage(data.language);
      }
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }, [i18n]);

  const fetchMeta = useCallback(async () => {
    try {
      const m = await invoke<[string, string, string[]][]>("get_available_models");
      setModels(m);
      const a = await invoke<[string, string][]>("get_approval_modes");
      setApprovalModes(a);
      const lm = await invoke<[string, string][]>("get_local_models");
      setLocalModels(lm);
    } catch (e) {
      console.error("Failed to load settings meta:", e);
    }
  }, []);

  useEffect(() => {
    if (!isOpen) return;
    fetchSettings();
    fetchMeta();
  }, [isOpen, fetchSettings, fetchMeta]);

  // 关闭面板时恢复主题为已保存的值（Cancel 或未 Save 的 preview）
  const prevIsOpenRef = useRef(isOpen);
  useEffect(() => {
    if (prevIsOpenRef.current && !isOpen) {
      const t = savedSettings.theme;
      if (t === "auto") {
        const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        document.documentElement.setAttribute("data-theme", prefersDark ? "dark" : "light");
      } else {
        document.documentElement.setAttribute("data-theme", t);
      }
    }
    prevIsOpenRef.current = isOpen;
  }, [isOpen, savedSettings]);

  const availableModels =
    models.find(([key]) => key === settings.provider)?.[2] ?? [];

  function handleProviderChange(provider: string) {
    setSettings((prev) => {
      const newModels = models.find(([key]) => key === provider)?.[2] ?? [];
      let nextModel = newModels[0] ?? prev.model;
      let nextLocalPath = prev.local_model_path;
      if (provider === "local" && localModels.length > 0) {
        nextModel = localModels[0][1];
        nextLocalPath = localModels[0][0];
      }
      return {
        ...prev,
        provider,
        model: nextModel,
        local_model_path: nextLocalPath,
      };
    });
  }

  function handleModelChange(model: string) {
    setSettings((prev) => {
      let localPath = prev.local_model_path;
      if (prev.provider === "local") {
        const found = localModels.find(([_, name]) => name === model);
        if (found) {
          localPath = found[0];
        }
      }
      return { ...prev, model, local_model_path: localPath };
    });
  }

  async function handleSave() {
    try {
      await invoke("save_settings", { settings });
      setSavedSettings(settings);
      if (settings.language) {
        i18n.changeLanguage(settings.language);
      }
      setToast(t("settings.saved"));
      setTimeout(() => setToast(""), 2000);
      try {
        await invoke("set_approval_mode", { mode: settings.approval_mode });
      } catch (e) {
        console.error("Failed to set approval mode:", e);
      }
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  function handleReset() {
    setSettings(savedSettings);
    const t = savedSettings.theme;
    if (t === "auto") {
      const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      document.documentElement.setAttribute("data-theme", prefersDark ? "dark" : "light");
    } else {
      document.documentElement.setAttribute("data-theme", t);
    }
  }

  if (!isOpen) return null;

  const isLocalProvider = settings.provider === "local";
  const hasLocalModels = localModels.length > 0;
  const localModelNames = localModels.map(([_, name]) => name);
  const displayModels = isLocalProvider ? localModelNames : availableModels;

  return (
    <div className="settings-panel">
      <div className="settings-panel-header">
        <h2>{t("settings.title")}</h2>
        <button
          className="settings-panel-close"
          onClick={onClose}
          aria-label={t("settings.close")}
        >
          ✕
        </button>
      </div>

      <div className="settings-panel-body">
        {/* Model Group */}
        <div className="settings-group">
          <h3>{t("settings.model")}</h3>
          <label className="settings-label">{t("settings.provider")}</label>
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

          <label className="settings-label">{t("settings.model")}</label>
          <select
            className="settings-select"
            value={settings.model}
            onChange={(e) => handleModelChange(e.target.value)}
          >
            {displayModels.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>

          {isLocalProvider && (
            <>
              <label className="settings-label">{t("settings.localModelPath")}</label>
              <input
                className="settings-input"
                type="text"
                readOnly
                value={settings.local_model_path ?? ""}
                placeholder={
                  hasLocalModels
                    ? t("settings.autoDetected")
                    : t("settings.noModels")
                }
              />
              {!hasLocalModels && (
                <p className="settings-hint settings-hint-warning">
                  {t("settings.noModels")}
                </p>
              )}
            </>
          )}
        </div>

        {/* Approval Group */}
        <div className="settings-group">
          <h3>{t("settings.approvalMode")}</h3>
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
          <h3>{t("settings.theme")}</h3>
          {[
            ["dark", t("settings.dark")],
            ["light", t("settings.light")],
            ["auto", t("settings.auto")],
          ].map(([key, display]) => (
            <label key={key} className="settings-radio-label">
              <input
                type="radio"
                name="theme"
                value={key}
                checked={settings.theme === key}
                onChange={(e) => {
                  const newTheme = e.target.value;
                  setSettings((prev) => ({ ...prev, theme: newTheme }));
                  // 立即应用主题预览
                  if (newTheme === "auto") {
                    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
                    document.documentElement.setAttribute("data-theme", prefersDark ? "dark" : "light");
                  } else {
                    document.documentElement.setAttribute("data-theme", newTheme);
                  }
                }}
              />
              <span>{display}</span>
            </label>
          ))}
        </div>

        {/* Language Group */}
        <div className="settings-group">
          <h3>{t("settings.language")}</h3>
          <select
            className="settings-select"
            value={settings.language ?? "zh"}
            onChange={(e) =>
              setSettings((prev) => ({ ...prev, language: e.target.value }))
            }
          >
            <option value="zh">中文</option>
            <option value="en">English</option>
          </select>
        </div>

        {/* Network Group */}
        <div className="settings-group">
          <h3>Network</h3>
          <label className="settings-label">{t("settings.networkProbeUrl")}</label>
          <input
            className="settings-input"
            type="text"
            value={settings.network_probe_url ?? ""}
            placeholder="1.1.1.1:443"
            onChange={(e) =>
              setSettings((prev) => ({
                ...prev,
                network_probe_url: e.target.value || undefined,
              }))
            }
          />
          <p className="settings-hint">
            TCP probe used for offline detection. Format: <code>host:port</code>.
            Leave empty to use the default.
          </p>
        </div>
      </div>

      <div className="settings-panel-footer">
        <button className="settings-btn settings-btn-primary" onClick={handleSave}>
          {t("settings.save")}
        </button>
        <button className="settings-btn settings-btn-secondary" onClick={handleReset}>
          {t("settings.reset")}
        </button>
        <button className="settings-btn settings-btn-secondary" onClick={onClose}>
          {t("settings.cancel")}
        </button>
        {toast && <span className="settings-toast">{toast}</span>}
      </div>
    </div>
  );
}

export default SettingsPanel;
