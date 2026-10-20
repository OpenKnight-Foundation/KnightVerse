import { test, expect } from "@playwright/test";

/**
 * E-2E coverage for #FE-96: the sign-in modal must be reachable from the
 * header and must actually call the auth API. We mock the backend so the test
 * is self-contained and does not depend on a running server.
 */


/** Build a fake JWT whose `exp` is far enough in the future to avoid refresh. */
function makeJwt(payload: Record<string, unknown>): string {
  const encode = (obj: Record<string, unknown>) =>
    Buffer.from(JSON.stringify(obj)).toString("base64url").replace(/=/g, "");
  const header = encode({ alg: "HS256", typ: "JWT" });
  const body = encode(payload);
  return `${header}.${body}.signature`;
}

test.describe("sign in", () => {
  test.beforeEach(async ({ page }) => {
    // Mock the login endpoint with a successful AuthResponse.
    await page.route("**/auth/login", (route) => {
      const accessToken = makeJwt({
        sub: 1,
        username: "tester",
        exp: Math.floor(Date.now() / 1000) + 3600,
      });
      return route.fulfill({
        status: 200,
        contentType: "application/json",
        body: JSON.stringify({
          access_token: accessToken,
          refresh_token: "refresh-token",
          token_type: "Bearer",
          expires_in: 3600,
          refresh_token_expires_in: 86400,
          user_id: 1,
          username: "tester",
          wallet_address: null,
        }),
      });
    });
  });

  test("signs in via the header modal against a mocked API", async ({ page }) => {
    await page.goto("/");

    // Open the modal from the header.
    await page.getByTestId("header-sign-in").click();
    const modal = page.getByTestId("sign-in-modal");
    await expect(modal).toBeVisible();

    // Fill and submit the login form.
    await page.getByLabel("Username").fill("tester");
    await page.getByLabel("Password").fill("secret-password");
    await page.getByTestId("sign-in-submit").click();

    // Modal closes and the header reflects the signed-in user.
    await expect(modal).not.toBeVisible();
    await expect(page.getByTestId("header-username")).toHaveText("tester");
  });

  test("shows an error when the API rejects the credentials", async ({ page }) => {
    await page.route("**/auth/login", (route) =>
      route.fulfill({
        status: 401,
        contentType: "application/json",
        body: JSON.stringify({ message: "Invalid credentials" }),
      }),
    );

    await page.goto("/");
    await page.getByTestId("header-sign-in").click();
    await page.getByLabel("Username").fill("tester");
    await page.getByLabel("Password").fill("wrong");
    await page.getByTestId("sign-in-submit").click();

    await expect(page.getByTestId("sign-in-error")).toHaveText("Invalid credentials");
  });
});
