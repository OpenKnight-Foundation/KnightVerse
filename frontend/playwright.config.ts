import { defineConfig } from "@playwright/test";

/**
 * Playwright configuration for KnightVerse frontend E2E (#1438).
 *
 * In CI we exercise the production build (`npm run build && npm run start`
 * or `vite preview` / Next start depending on the app). Locally the dev
 * server is reused when already running.
 */
const isCI = !!process.env.CI;

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  retries: isCI ? 2 : 1,
  forbidOnly: isCI,
  reporter: isCI ? [["github"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: process.env.BASE_URL ?? "http://localhost:3000",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  webServer: {
    // Production build in CI; dev server for local iteration.
    command: isCI ? "npm run start" : "npm run dev",
    port: 3000,
    reuseExistingServer: !isCI,
    timeout: 120_000,
    env: {
      // Test-only escrow account so the staking spec can complete a deposit.
      NEXT_PUBLIC_STAKING_ESCROW_ADDRESS:
        process.env.NEXT_PUBLIC_STAKING_ESCROW_ADDRESS ??
        "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA",
    },
  },
  projects: [{ name: "chromium", use: { browserName: "chromium" } }],
});
