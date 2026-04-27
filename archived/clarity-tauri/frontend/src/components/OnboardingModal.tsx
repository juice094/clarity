import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { CheckCircle, AlertTriangle, XCircle, Download } from "lucide-react";

export interface LaunchStatus {
  has_local_model: boolean;
  network_available: boolean;
  configured: boolean;
  needs_onboarding: boolean;
  first_launch: boolean;
}

interface OnboardingModalProps {
  status: LaunchStatus;
  onOpenSettings: () => void;
  onDismiss: () => void;
}

const RECOMMENDED_MODEL = {
  repo: "unsloth/DeepSeek-R1-Distill-Qwen-1.5B-GGUF",
  file: "DeepSeek-R1-Distill-Qwen-1.5B-Q4_K_M.gguf",
};

function OnboardingModal({ status, onOpenSettings, onDismiss }: OnboardingModalProps) {
  const { t } = useTranslation();
  const [downloading, setDownloading] = useState(false);
  const [downloadProgress, setDownloadProgress] = useState(0);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    listen<{ bytes: number; total: number | null }>("download:progress", (event) => {
      const { bytes, total } = event.payload;
      setDownloadProgress(total && total > 0 ? bytes / total : 0);
    }).then((u) => unlisteners.push(u));
    listen("download:complete", () => {
      setDownloading(false);
      setDownloadProgress(1);
    }).then((u) => unlisteners.push(u));
    return () => {
      unlisteners.forEach((u) => u());
    };
  }, []);

  async function handleDownload() {
    setDownloading(true);
    setDownloadProgress(0);
    try {
      await invoke("download_model", {
        repoId: RECOMMENDED_MODEL.repo,
        filename: RECOMMENDED_MODEL.file,
      });
    } catch (e) {
      console.error("Model download failed:", e);
      setDownloadStatus("error");
      setDownloading(false);
    }
  }

  const canProceed = status.configured;
  const canDownload = !status.has_local_model && status.network_available;

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-modal">
        <h1>{t("onboarding.title", "Welcome to Clarity")}</h1>
        <p className="onboarding-subtitle">
          {t("onboarding.subtitle", "Let's get you set up.")}
        </p>

        <div className="onboarding-status">
          <div className={`status-item ${status.network_available ? "ok" : "warn"}`}>
            <span className="status-icon">{status.network_available ? <CheckCircle size={16} /> : <AlertTriangle size={16} />}</span>
            <span className="status-label">
              {status.network_available
                ? t("onboarding.networkOk", "Network available")
                : t("onboarding.networkOffline", "Network offline — local mode only")}
            </span>
          </div>

          <div className={`status-item ${status.has_local_model ? "ok" : "warn"}`}>
            <span className="status-icon">{status.has_local_model ? <CheckCircle size={16} /> : <AlertTriangle size={16} />}</span>
            <span className="status-label">
              {status.has_local_model
                ? t("onboarding.localModelOk", "Local model found")
                : t("onboarding.localModelMissing", "No local model found")}
            </span>
          </div>

          <div className={`status-item ${status.configured ? "ok" : "warn"}`}>
            <span className="status-icon">{status.configured ? <CheckCircle size={16} /> : <XCircle size={16} />}</span>
            <span className="status-label">
              {status.configured
                ? t("onboarding.configured", "Ready to chat")
                : t("onboarding.notConfigured", "Model / provider not configured")}
            </span>
          </div>
        </div>

        {!status.configured && (
          <div className="onboarding-hint">
            {status.network_available
              ? t(
                  "onboarding.hintCloud",
                  "Select a provider and enter your API key in Settings to start chatting."
                )
              : t(
                  "onboarding.hintOffline",
                  "You are offline. Place a .gguf model in ~/models/ and select Local (GGUF) provider."
                )}
          </div>
        )}

        <div className="onboarding-actions">
          {canDownload && (
            <button
              className="onboarding-btn primary"
              onClick={handleDownload}
              disabled={downloading}
              title={RECOMMENDED_MODEL.file}
            >
              <Download size={14} />
              {downloading
                ? `${t("onboarding.downloading", "Downloading")} ${Math.round(downloadProgress * 100)}%`
                : t("onboarding.downloadModel", "Download Model (~1GB)")}
            </button>
          )}
          <button
            className={`onboarding-btn ${canDownload ? "secondary" : "primary"}`}
            onClick={() => { onOpenSettings(); onDismiss(); }}
          >
            {t("onboarding.openSettings", "Configure Model")}
          </button>
          {canProceed && (
            <button
              className="onboarding-btn secondary"
              onClick={onDismiss}
            >
              {t("onboarding.startChat", "Start Chatting")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export default OnboardingModal;
