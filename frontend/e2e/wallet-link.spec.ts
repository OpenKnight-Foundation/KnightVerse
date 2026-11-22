import { test, expect, type Page } from "@playwright/test";

const WALLET = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";
const CHALLENGE = "KnightVerse wallet link challenge";
const SIGNATURE = "e2e-signed-challenge";

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "GET, POST, DELETE, OPTIONS",
};

type VerifyResponse = { status: number; body: unknown };

function makeJwt(): string {
  const encode = (obj: Record<string, unknown>) =>
    Buffer.from(JSON.stringify(obj)).toString("base64url").replace(/=/g, "");
  return `${encode({ alg: "HS256", typ: "JWT" })}.${encode({
    sub: "1",
    user_id: 1,
    username: "tester",
    exp: Math.floor(Date.now() / 1000) + 3600,
  })}.signature`;
}

async function setup(
  page: Page,
  {
    linked = null,
    verify = { status: 200, body: { wallet_address: WALLET } },
  }: { linked?: string | null; verify?: VerifyResponse } = {},
): Promise<string[]> {
  const requests: string[] = [];

  await page.addInitScript(
    ({ wallet }) => {
      window.localStorage.setItem("freighter_address", wallet);
      (window as unknown as { freighterApi: unknown }).freighterApi = {
        getPublicKey: async () => wallet,
        signMessage: async () => "e2e-signed-challenge",
        signTransaction: async (xdr: string) => xdr,
      };
    },
    { wallet: WALLET },
  );

  // The access token lives in memory only (#FE-166): sign in through the
  // silent refresh on mount, which sends the session cookie cross-origin.
  await page.route("**/v1/auth/refresh", (route) => {
    const origin = route.request().headers()["origin"] ?? "*";
    const headers = {
      "Access-Control-Allow-Origin": origin,
      "Access-Control-Allow-Credentials": "true",
      "Access-Control-Allow-Headers": "*",
      "Access-Control-Allow-Methods": "POST, OPTIONS",
    };
    if (route.request().method() === "OPTIONS") {
      return route.fulfill({ status: 204, headers });
    }
    return route.fulfill({ headers, json: { access_token: makeJwt() } });
  });

  await page.route("**/v1/players/me/wallet/challenge", (route) => {
    requests.push("challenge");
    if (route.request().method() === "OPTIONS") {
      return route.fulfill({ status: 204, headers: CORS_HEADERS });
    }
    return route.fulfill({ headers: CORS_HEADERS, json: { message: CHALLENGE } });
  });

  await page.route("**/v1/players/me/wallet/verify", (route) => {
    requests.push("verify");
    if (route.request().method() === "OPTIONS") {
      return route.fulfill({ status: 204, headers: CORS_HEADERS });
    }
    return route.fulfill({
      status: verify.status,
      headers: CORS_HEADERS,
      json: verify.body,
    });
  });

  await page.route("**/v1/players/me/wallet", (route) => {
    const method = route.request().method();
    if (method === "OPTIONS") {
      return route.fulfill({ status: 204, headers: CORS_HEADERS });
    }
    // The settings panel loads the linked wallet after the silent restore.
    if (method === "GET") {
      return route.fulfill({ headers: CORS_HEADERS, json: { wallet_address: linked } });
    }
    requests.push("unlink");
    return route.fulfill({ headers: CORS_HEADERS, json: { success: true } });
  });

  return requests;
}

test.describe("wallet linking", () => {
  test("links the connected Freighter wallet", async ({ page }) => {
    const requests = await setup(page);
    await page.goto("/settings");

    const panel = page.getByRole("region", { name: "Linked wallet" });
    await expect(panel.getByText("No wallet linked yet.")).toBeVisible();

    const signedRequest = page.waitForRequest(
      (request) =>
        request.url().includes("/v1/players/me/wallet/verify") &&
        request.method() === "POST",
    );
    await panel.getByRole("button", { name: /^Link / }).click();
    const request = await signedRequest;

    expect(request.postDataJSON()).toEqual({
      wallet_address: WALLET,
      signature: SIGNATURE,
    });
    await expect(panel.getByText(/Linked:/)).toContainText(
      `${WALLET.slice(0, 6)}…${WALLET.slice(-6)}`,
    );
    await expect(panel.getByRole("button", { name: "Unlink" })).toBeVisible();
    expect(requests).toEqual(["challenge", "verify"]);
  });

  test("surfaces a backend verification error", async ({ page }) => {
    const requests = await setup(page, {
      verify: { status: 500, body: { message: "Wallet verification failed" } },
    });
    await page.goto("/settings");

    const panel = page.getByRole("region", { name: "Linked wallet" });
    await panel.getByRole("button", { name: /^Link / }).click();

    await expect(panel.getByRole("alert")).toHaveText("Wallet verification failed");
    await expect(panel.getByText("No wallet linked yet.")).toBeVisible();
    expect(requests).toEqual(["challenge", "verify"]);
  });

  test("unlinks the wallet", async ({ page }) => {
    const requests = await setup(page, { linked: WALLET });
    await page.goto("/settings");

    const panel = page.getByRole("region", { name: "Linked wallet" });
    await expect(panel.getByText(/Linked:/)).toBeVisible();

    const unlinkRequest = page.waitForRequest(
      (request) =>
        request.url().endsWith("/v1/players/me/wallet") &&
        request.method() === "DELETE",
    );
    await panel.getByRole("button", { name: "Unlink" }).click();
    await unlinkRequest;

    await expect(panel.getByText("No wallet linked yet.")).toBeVisible();
    await expect(panel.getByRole("button", { name: "Unlink" })).toHaveCount(0);
    expect(requests).toEqual(["unlink"]);
  });
});
