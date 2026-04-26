import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import OnboardingModal, { type LaunchStatus } from "./OnboardingModal";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, defaultValue?: string) => defaultValue || key,
  }),
}));

const baseStatus: LaunchStatus = {
  has_local_model: true,
  network_available: true,
  configured: true,
  needs_onboarding: true,
  first_launch: true,
};

describe("OnboardingModal smoke test", () => {
  it("renders with all status indicators when configured", () => {
    render(
      <OnboardingModal
        status={baseStatus}
        onOpenSettings={vi.fn()}
        onDismiss={vi.fn()}
      />
    );
    expect(screen.getByText("Welcome to Clarity")).toBeInTheDocument();
    expect(screen.getByText("Network available")).toBeInTheDocument();
    expect(screen.getByText("Local model found")).toBeInTheDocument();
    expect(screen.getByText("Ready to chat")).toBeInTheDocument();
    expect(screen.getByText("Configure Model")).toBeInTheDocument();
    expect(screen.getByText("Start Chatting")).toBeInTheDocument();
  });

  it("renders with warning indicators when not configured", () => {
    render(
      <OnboardingModal
        status={{ ...baseStatus, configured: false, has_local_model: false }}
        onOpenSettings={vi.fn()}
        onDismiss={vi.fn()}
      />
    );
    expect(screen.getByText("Model / provider not configured")).toBeInTheDocument();
    expect(screen.getByText("No local model found")).toBeInTheDocument();
    expect(screen.queryByText("Start Chatting")).not.toBeInTheDocument();
  });

  it("calls onOpenSettings and onDismiss when configure button clicked", () => {
    const onOpenSettings = vi.fn();
    const onDismiss = vi.fn();
    render(
      <OnboardingModal
        status={baseStatus}
        onOpenSettings={onOpenSettings}
        onDismiss={onDismiss}
      />
    );
    fireEvent.click(screen.getByText("Configure Model"));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
