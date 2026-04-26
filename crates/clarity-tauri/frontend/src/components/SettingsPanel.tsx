import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { X, Download } from "lucide-react";

const FALLBACK_MODELS: [string, string, string[]][] = [
  ["openai", "OpenAI", ["gpt-4o", "gpt-4o-mini", "o3-mini"]],
  ["anthropic", "Anthropic", ["claude-3-sonnet", "claude-3-opus"]],
  ["kimi", "Kimi", ["kimi-k2-07132k", "kimi-latest"]],
  ["ollama", "Ollama", ["llama3.2", "qwen2.5"]],
  ["local", "Local (GGUF)", ["No models found — place .gguf in ~/models/"]],
];

export interface GuiSettings {
  model: string;
  provider: string;
  approval_mode: string;
  theme: string;
  local_model_path?: string;
  network_probe_url?: string;
  language?: string;
  api_key?: string;
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
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [downloadRepo, setDownloadRepo] = useState("unsloth/DeepSeek-R1-Distill-Qwen-1.5B-GGUF");
  const [downloadFile, setDownloadFile] = useState("DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf");
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadStatus, setDownloadStatus] = useState<"idle" | "downloading" | "done" | "error">("idle");

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

  // Listen for model download progress events
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    listen<{ bytes: number; total: number | null; filename: string }>("download:progress", (event) => {
      const { bytes, total } = event.payload;
      setDownloadProgress(total && total > 0 ? bytes / total : 0);
    }).then((u) => unlisteners.push(u));
    listen<{ path: string; filename: string }>("download:complete", () => {
      setDownloadStatus("done");
      setDownloadProgress(1);
      fetchMeta();
      setTimeout(() => setDownloadStatus("idle"), 3000);
    }).then((u) => unlisteners.push(u));
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [fetchMeta]);

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

  const modelSource = models.length > 0 ? models : FALLBACK_MODELS;
  const availableModels =
    modelSource.find(([key]) => key === settings.provider)?.[2] ?? [];

  function scheduleAutoSave() {
    if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
    autoSaveTimerRef.current = setTimeout(() => {
      handleSave();
      autoSaveTimerRef.current = null;
    }, 1000);
  }

  function handleProviderChange(provider: string) {
    setSettings((prev) => {
      const newModels = modelSource.find(([key]) => key === provider)?.[2] ?? [];
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
    scheduleAutoSave();
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
    scheduleAutoSave();
  }

  async function handleSave() {
    // Cancel any pending auto-save to avoid double-save
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
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
        setToast(t("settings.saveFailed"));
      }
      try {
        await invoke("reload_llm");
      } catch (e) {
        console.error("Failed to reload LLM:", e);
        setToast(t("settings.saveFailed"));
      }
    } catch (e) {
      console.error("Failed to save settings:", e);
      setToast(t("settings.saveFailed"));
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
          <X size={16} />
        </button>
      </div>

      <div className="settings-panel-body">
        {/* Model Group */}
        <div className="settings-group">
          <h3>{t("settings.model")}</h3>
          <label className="settings-label" htmlFor="settings-provider">{t("settings.provider")}</label>
          <select
            id="settings-provider"
            className="settings-select"
            value={settings.provider}
            onChange={(e) => handleProviderChange(e.target.value)}
          >
            {modelSource.map(([key, display]) => (
              <option key={key} value={key}>
                {display}
              </option>
            ))}
          </select>

          <label className="settings-label" htmlFor="settings-model">{t("settings.model")}</label>
          <select
            id="settings-model"
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

          {!isLocalProvider && (
            <>
              <label className="settings-label" htmlFor="settings-api-key">API Key</label>
              <input
                id="settings-api-key"
                className="settings-input"
                type="password"
                value={settings.api_key ?? ""}
                placeholder="sk-..."
                onChange={(e) =>
                  setSettings((prev) => ({
                    ...prev,
                    api_key: e.target.value || undefined,
                  }))
                }
              />
              <p className="settings-hint">
                Stored locally in %APPDATA%/clarity/gui-settings.json
              </p>
            </>
          )}

          {isLocalProvider && (
            <>
              <label className="settings-label" htmlFor="settings-local-model-path">{t("settings.localModelPath")}</label>
              <input
                id="settings-local-model-path"
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

              <div className="download-section">
                <label className="settings-label" htmlFor="settings-download-repo">Download from HuggingFace</label>
                <input
                  id="settings-download-repo"
                  className="settings-input"
                  type="text"
                  placeholder="unsloth/DeepSeek-R1-Distill-Qwen-1.5B-GGUF"
                  value={downloadRepo}
                  onChange={(e) => setDownloadRepo(e.target.value)}
                  disabled={downloadStatus === "downloading"}
                />
                <input
                  id="settings-download-file"
                  className="settings-input"
                  type="text"
                  placeholder="DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf"
                  value={downloadFile}
                  onChange={(e) => setDownloadFile(e.target.value)}
                  disabled={downloadStatus === "downloading"}
                />
                {downloadStatus === "downloading" && (
                  <div className="download-progress">
                    <progress value={downloadProgress} max={1} />
                    <span className="download-progress-text">
                      {Math.round(downloadProgress * 100)}%
                    </span>
                  </div>
                )}
                <button
                  className="download-btn"
                  onClick={async () => {
                    if (!downloadRepo.trim() || !downloadFile.trim()) return;
                    setDownloadStatus("downloading");
                    setDownloadProgress(0);
                    try {
                      await invoke("download_model", {
                        repoId: downloadRepo.trim(),
                        filename: downloadFile.trim(),
                      });
                    } catch (e) {
                      console.error("Download failed:", e);
                      setDownloadStatus("error");
                      setTimeout(() => setDownloadStatus("idle"), 3000);
                    }
                  }}
                  disabled={downloadStatus === "downloading" || !downloadRepo.trim() || !downloadFile.trim()}
                >
                  <Download size={14} />
                  {downloadStatus === "downloading"
                    ? "Downloading…"
                    : downloadStatus === "done"
                    ? "Downloaded"
                    : downloadStatus === "error"
                    ? "Failed"
                    : "Download Model"}
                </button>
              </div>
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
          <label className="settings-label" htmlFor="settings-language">{t("settings.language")}</label>
          <select
            id="settings-language"
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
          <label className="settings-label" htmlFor="settings-network-probe">{t("settings.networkProbeUrl")}</label>
          <input
            id="settings-network-probe"
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
