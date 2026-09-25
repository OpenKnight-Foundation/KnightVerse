import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  timeout: 30_000,
  retries: 1,
  use: {
    baseURL: process.env.BASE_URL ?? "http://localhost:3000",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
  },
  webServer: {
    command: "npm run dev",
    port: 3000,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
    env: {
      // Test-only escrow account so the staking spec can complete a deposit.
      NEXT_PUBLIC_STAKING_ESCROW_ADDRESS:
        process.env.NEXT_PUBLIC_STAKING_ESCROW_ADDRESS ??
        "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA",
    },
  },
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
  ],
});
