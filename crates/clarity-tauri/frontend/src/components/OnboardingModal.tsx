import { useTranslation } from "react-i18next";

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

function OnboardingModal({ status, onOpenSettings, onDismiss }: OnboardingModalProps) {
  const { t } = useTranslation();

  const canProceed = status.configured;

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-modal">
        <h1>{t("onboarding.title", "Welcome to Clarity")}</h1>
        <p className="onboarding-subtitle">
          {t("onboarding.subtitle", "Let's get you set up.")}
        </p>

        <div className="onboarding-status">
          <div className={`status-item ${status.network_available ? "ok" : "warn"}`}>
            <span className="status-icon">{status.network_available ? "✅" : "⚠️"}</span>
            <span className="status-label">
              {status.network_available
                ? t("onboarding.networkOk", "Network available")
                : t("onboarding.networkOffline", "Network offline — local mode only")}
            </span>
          </div>

          <div className={`status-item ${status.has_local_model ? "ok" : "warn"}`}>
            <span className="status-icon">{status.has_local_model ? "✅" : "⚠️"}</span>
            <span className="status-label">
              {status.has_local_model
                ? t("onboarding.localModelOk", "Local model found")
                : t("onboarding.localModelMissing", "No local model found")}
            </span>
          </div>

          <div className={`status-item ${status.configured ? "ok" : "warn"}`}>
            <span className="status-icon">{status.configured ? "✅" : "❌"}</span>
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
                  "You can use a cloud provider (OpenAI / Anthropic) or download a local GGUF model."
                )
              : t(
                  "onboarding.hintOffline",
                  "You are offline. Please place a .gguf model file in ~/models/ to use local mode."
                )}
          </div>
        )}

        <div className="onboarding-actions">
          <button
            className="onboarding-btn primary"
            onClick={onOpenSettings}
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
