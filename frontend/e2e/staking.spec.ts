import { test, expect, type Page, type Route } from "@playwright/test";

// Valid Stellar public key used only as a test fixture.
const PLAYER_ADDRESS = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";
// Circle's testnet USDC issuer (matches TOKEN_ISSUERS.testnet.USDC).
const TESTNET_USDC_ISSUER = "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5";
const TX_HASH = "e2e0000000000000000000000000000000000000000000000000000000000001";

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
};

type MockWallet = { submittedTransactions: number; mainnetRequests: string[] };
type MockBalances = { xlm: string; usdc?: string };

/**
 * Connects a mock Freighter wallet and stubs CoinGecko + Horizon so the staking
 * flow runs end-to-end without touching a real network.
 */
async function setUpMockWallet(page: Page, { xlm, usdc }: MockBalances): Promise<MockWallet> {
  const wallet: MockWallet = { submittedTransactions: 0, mainnetRequests: [] };

  await page.addInitScript((address) => {
    window.localStorage.setItem("freighter_address", address);
    (window as unknown as { freighterApi: unknown }).freighterApi = {
      getPublicKey: async () => address,
      // Echo the XDR back as "signed"; Horizon is mocked so no real signature is needed.
      signTransaction: async (xdr: string) => xdr,
    };
  }, PLAYER_ADDRESS);

  const context = page.context();

  await context.route("https://api.coingecko.com/**", (route) =>
    route.fulfill({
      headers: CORS_HEADERS,
      json: { stellar: { usd: 0.1 }, "usd-coin": { usd: 1 }, "euro-coin": { usd: 1.08 } },
    }),
  );

  // FE-70: the app targets testnet by default, so mainnet Horizon must never be queried.
  await context.route("https://horizon.stellar.org/**", (route) => {
    wallet.mainnetRequests.push(route.request().url());
    return route.fulfill({ status: 404, headers: CORS_HEADERS, json: { status: 404 } });
  });

  await context.route("https://horizon-testnet.stellar.org/**", (route: Route) => {
    const request = route.request();
    const { pathname } = new URL(request.url());
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers: CORS_HEADERS });

    if (pathname.startsWith("/accounts/")) {
      const accountId = decodeURIComponent(pathname.split("/")[2]);
      return route.fulfill({
        headers: CORS_HEADERS,
        json: {
          id: accountId,
          account_id: accountId,
          sequence: "100",
          subentry_count: 0,
          thresholds: { low_threshold: 0, med_threshold: 0, high_threshold: 0 },
          flags: { auth_required: false, auth_revocable: false, auth_immutable: false },
          balances: [
            { asset_type: "native", balance: xlm },
            ...(usdc
              ? [{ asset_type: "credit_alphanum4", asset_code: "USDC", asset_issuer: TESTNET_USDC_ISSUER, balance: usdc }]
              : []),
          ],
          signers: [{ key: accountId, weight: 1, type: "ed25519_public_key" }],
          data: {},
        },
      });
    }
    if (pathname === "/fee_stats") {
      return route.fulfill({ headers: CORS_HEADERS, json: { last_ledger_base_fee: "100" } });
    }
    if (pathname === "/transactions" && request.method() === "POST") {
      wallet.submittedTransactions += 1;
      return route.fulfill({ headers: CORS_HEADERS, json: { hash: TX_HASH, successful: true, ledger: 1 } });
    }
    return route.fulfill({ status: 404, headers: CORS_HEADERS, json: { status: 404 } });
  });

  return wallet;
}

test.describe("Staking deposit", () => {
  test("stakes successfully against a funded testnet wallet", async ({ page }) => {
    const wallet = await setUpMockWallet(page, { xlm: "1000.0000000" });
    await page.goto("/stake");

    await page.getByRole("button", { name: /stake & play/i }).click();
    const modal = page.getByRole("dialog", { name: /confirm your deposit/i });
    await expect(modal).toBeVisible();
    await expect(modal.getByText(/staking on stellar testnet/i)).toBeVisible();

    await modal.getByLabel("Match stake").fill("10");
    await expect(modal.getByText("10.3500 XLM")).toBeVisible();
    await expect(modal.getByText(/insufficient balance/i)).toBeHidden();

    const confirm = modal.getByRole("button", { name: /confirm stake/i });
    await expect(confirm).toBeEnabled();
    await confirm.click();

    await expect(modal).toBeHidden();
    const confirmation = page.getByText(/Stake confirmed: 10\.3500 XLM/);
    await expect(confirmation).toBeVisible();
    await expect(confirmation).toContainText(TX_HASH);
    expect(wallet.submittedTransactions).toBe(1);
    expect(wallet.mainnetRequests).toEqual([]);
  });

  test("rejects a stake that exceeds the wallet balance", async ({ page }) => {
    const wallet = await setUpMockWallet(page, { xlm: "5.0000000" });
    await page.goto("/stake");

    await page.getByRole("button", { name: /stake & play/i }).click();
    const modal = page.getByRole("dialog", { name: /confirm your deposit/i });
    await expect(modal).toBeVisible();

    await modal.getByLabel("Match stake").fill("10");
    await expect(modal.getByText(/insufficient balance\. available: 5\.0000 xlm/i)).toBeVisible();
    await expect(modal.getByRole("button", { name: /confirm stake/i })).toBeDisabled();

    expect(wallet.submittedTransactions).toBe(0);
    expect(wallet.mainnetRequests).toEqual([]);
  });

  test("checks a USDC stake against the USDC balance, not XLM", async ({ page }) => {
    // Plenty of XLM, little USDC: a USDC stake must be rejected.
    await setUpMockWallet(page, { xlm: "1000.0000000", usdc: "2.0000000" });
    await page.goto("/stake");

    await page.getByRole("button", { name: /stake & play/i }).click();
    const modal = page.getByRole("dialog", { name: /confirm your deposit/i });
    await modal.getByRole("button", { name: "USDC", exact: true }).click();
    await modal.getByLabel("Match stake").fill("10");

    await expect(modal.getByText(/insufficient balance\. available: 2\.0000 usdc/i)).toBeVisible();
    await expect(modal.getByRole("button", { name: /confirm stake/i })).toBeDisabled();

    // Switching back to XLM validates against the (sufficient) XLM balance.
    await modal.getByRole("button", { name: "XLM", exact: true }).click();
    await expect(modal.getByText(/insufficient balance/i)).toBeHidden();
    await expect(modal.getByRole("button", { name: /confirm stake/i })).toBeEnabled();
  });

  test("accepts a USDC stake when USDC is funded even if XLM is low", async ({ page }) => {
    await setUpMockWallet(page, { xlm: "1.0000000", usdc: "500.0000000" });
    await page.goto("/stake");

    await page.getByRole("button", { name: /stake & play/i }).click();
    const modal = page.getByRole("dialog", { name: /confirm your deposit/i });
    await modal.getByRole("button", { name: "USDC", exact: true }).click();
    await modal.getByLabel("Match stake").fill("10");

    await expect(modal.getByText("10.3500 USDC")).toBeVisible();
    await expect(modal.getByText(/insufficient balance/i)).toBeHidden();
    await expect(modal.getByRole("button", { name: /confirm stake/i })).toBeEnabled();
  });
});
