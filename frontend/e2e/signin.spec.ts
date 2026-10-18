import { test, expect } from "@playwright/test";

const API_BASE = process.env.PLAYWRIGHT_API_BASE ?? "http://localhost:8000";

const fakeAuthResponse = {
  access_token: "fake-access-token",
  refresh_token: "fake-refresh-token",
  token_type: "bearer",
  expires_in: 3600,
  refresh_token_expires_in: 86400,
  user_id: 1,
  username: "testuser",
  wallet_address: null,
};

test.describe("Sign in flow", () => {
  test.beforeEach(async ({ page }) => {
    await page.route(`${API_BASE}/auth/login`, async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(fakeAuthResponse),
      });
    });
  });

  test("signs in against mocked API", async ({ page }) => {
    await page.goto("/");

    // Open the sign-in modal from the header.
    await page.getByRole("button", { name: /sign in/i }).first().click();

    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();

    await dialog.getByLabel("Username").fill("testuser");
    await dialog.getByLabel("Password").fill("secret-password");
    await dialog.getByRole("button", { name: /^sign in$/i }).click();

    await expect(dialog).not.toBeVisible();

    // Session is persisted in localStorage and the user is authenticated.
    const token = await page.evaluate(() => localStorage.getItem("access_token"));
    expect(token).toBe("fake-access-token");
  });

  test("shows an error on invalid credentials", async ({ page }) => {
    await page.route(`${API_BASE}/auth/login`, async (route) => {
      await route.fulfill({
        status: 401,
        contentType: "application/json",
        body: JSON.stringify({ message: "Invalid credentials" }),
      });
    });

    await page.goto("/");
    await page.getByRole("button", { name: /sign in/i }).first().click();

    const dialog = page.getByRole("dialog");
    await dialog.getByLabel("Username").fill("testuser");
    await dialog.getByLabel("Password").fill("wrong");
    await dialog.getByRole("button", { name: /^sign in$/i }).click();

    await expect(dialog.getByRole("alert")).toHaveText("Invalid credentials");
  });
});
