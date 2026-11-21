import { test, expect } from "@playwright/test";


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
    await page.route("**/auth/login", async (route) => {
      await route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify(fakeAuthResponse),
      });
    });
    // No session cookie yet on a fresh page load, so the silent restore on
    // mount is expected to fail (#FE-166).
    await page.route("**/auth/refresh", async (route) => {
      await route.fulfill({
        status: 401,
        contentType: "application/json",
        body: JSON.stringify({ message: "Refresh token missing" }),
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

    // The access token is kept in memory only, never written to storage,
    // so an XSS payload can't read it back out of localStorage (#FE-166).
    const storedToken = await page.evaluate(() => localStorage.getItem("access_token"));
    const storedUser = await page.evaluate(() => localStorage.getItem("user"));
    expect(storedToken).toBeNull();
    expect(storedUser).toBeNull();

    // The user is still authenticated — e.g. the sign-in button is gone.
    await expect(page.getByRole("button", { name: /sign in/i })).toHaveCount(0);
  });

  test("shows an error on invalid credentials", async ({ page }) => {
    await page.route("**/auth/login", async (route) => {
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
