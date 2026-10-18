import { test, expect, type Page } from "@playwright/test";

// Valid Stellar public keys used only as fixtures.
const ME = "GAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQDZ7H";
const OPPONENT = "GABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEJXA";
const REQUEST_ID = "6f1c5a2e-0000-4000-8000-000000000001";
const MATCH_ID = "6f1c5a2e-0000-4000-8000-0000000000aa";

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
};

type Calls = { joins: { url: string; body: Record<string, unknown> }[]; statusPolls: number };

/**
 * Mocks the backend matchmaking API (#FE-98): the request waits in the queue
 * for one poll, then `/status` reports the match and `/match` assigns colours.
 */
async function mockMatchmakingApi(page: Page): Promise<Calls> {
  const calls: Calls = { joins: [], statusPolls: 0 };

  await page.addInitScript((address) => {
    window.localStorage.setItem("freighter_address", address);
  }, ME);

  await page.context().route(/\/v1\/matchmaking\//, (route) => {
    const request = route.request();
    const { pathname } = new URL(request.url());
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers: CORS_HEADERS });

    if (/\/join-(casual|rated-free)$/.test(pathname)) {
      calls.joins.push({ url: pathname, body: request.postDataJSON() });
      return route.fulfill({
        headers: CORS_HEADERS,
        json: { status: "Added to queue", match_id: null, request_id: REQUEST_ID },
      });
    }
    if (pathname.endsWith(`/status/${REQUEST_ID}`)) {
      calls.statusPolls += 1;
      return route.fulfill({
        headers: CORS_HEADERS,
        json:
          calls.statusPolls === 1
            ? { status: "In queue", queue_status: { request_id: REQUEST_ID, position: 1 } }
            : { status: "matched", queue_status: null, match_id: MATCH_ID },
      });
    }
    if (pathname.endsWith(`/match/${MATCH_ID}`)) {
      return route.fulfill({
        headers: CORS_HEADERS,
        json: {
          id: MATCH_ID,
          player1: { wallet_address: OPPONENT, elo: 1200 },
          player2: { wallet_address: ME, elo: 1200 },
        },
      });
    }
    return route.fulfill({ status: 404, headers: CORS_HEADERS, json: { status: "not found" } });
  });

  return calls;
}

test.describe("Online matchmaking", () => {
  // The dev server compiles /play/[slug] on first visit, which can take a while.
  test.setTimeout(90_000);

  test("joins the casual queue, polls status and opens the matched game", async ({ page }) => {
    const calls = await mockMatchmakingApi(page);
    await page.goto("/");

    await page.getByRole("button", { name: /play online/i }).click();
    await page.getByRole("button", { name: /casual match/i }).click();

    await expect(page).toHaveURL(new RegExp(`/play/${MATCH_ID}$`), { timeout: 60_000 });

    expect(calls.joins).toHaveLength(1);
    expect(calls.joins[0].url).toMatch(/\/v1\/matchmaking\/join-casual$/);
    expect(calls.joins[0].body).toMatchObject({ wallet_address: ME, elo: 1200 });
    expect(calls.statusPolls).toBeGreaterThanOrEqual(2);
  });

  test("uses the rated queue for a rated match", async ({ page }) => {
    const calls = await mockMatchmakingApi(page);
    await page.goto("/");

    await page.getByRole("button", { name: /play online/i }).click();
    await page.getByRole("button", { name: /rated match/i }).click();

    await expect(page).toHaveURL(new RegExp(`/play/${MATCH_ID}$`), { timeout: 60_000 });
    expect(calls.joins[0].url).toMatch(/\/v1\/matchmaking\/join-rated-free$/);
  });
});
