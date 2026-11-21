import React from "react";
import { describe, it, expect, vi } from "vitest";
import { render } from "@testing-library/react";
import ClientRoot from "@/components/ClientRoot";

vi.mock("@/components/GameSidebar", () => ({
  GameSidebar: () => <nav data-testid="sidebar" />,
}));
vi.mock("@/components/TransactionStatusIndicator", () => ({
  TransactionStatusIndicator: () => null,
}));
vi.mock("@/components/AuthButton", () => ({
  AuthButton: () => null,
}));
vi.mock("@/components/notifications/NotificationBell", () => ({
  NotificationBell: () => null,
}));
vi.mock("@/context/NotificationsContext", () => ({
  NotificationsProvider: ({ children }: { children: React.ReactNode }) => (
    <>{children}</>
  ),
}));

describe("ClientRoot theming", () => {
  it("uses theme tokens for the app shell instead of hardcoded dark colors", () => {
    const { container } = render(
      <ClientRoot>
        <div>content</div>
      </ClientRoot>
    );

    const shell = container.querySelector("div.flex.h-screen");
    expect(shell).not.toBeNull();
    expect(shell?.className).toContain("bg-background");
    expect(shell?.className).toContain("text-foreground");
    expect(shell?.className).not.toMatch(/gray-9|gray-8|text-white/);
  });

  it("uses a theme-aware border on the mobile header bar", () => {
    const { container } = render(
      <ClientRoot>
        <div>content</div>
      </ClientRoot>
    );

    const mobileHeader = container.querySelector(".md\\:hidden.flex.items-center.p-4");
    expect(mobileHeader).not.toBeNull();
    expect(mobileHeader?.className).toContain("border-border");
    expect(mobileHeader?.className).not.toMatch(/gray-8/);
  });
});
