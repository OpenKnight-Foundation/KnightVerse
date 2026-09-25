import { test, expect, type Page } from "@playwright/test";

const TOURNAMENT = {
  id: "t-e2e-open",
  name: "E2E Open",
  format: "SingleElimination",
  status: "Registration",
  participants: [],
  matches: [],
  total_rounds: 0,
  winner_id: null,
  created_at: "2026-09-01T00:00:00Z",
  started_at: null,
  completed_at: null,
};

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "http://localhost:3000",
  "Access-Control-Allow-Credentials": "true",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
};

/** Mocks the tournaments API; like the backend, a second signup is rejected with 409. */
async function mockTournamentApi(page: Page) {
  const registrations = { count: 0 };

  await page.context().route(/\/v1\/tournaments(\/.*)?$/, (route) => {
    const request = route.request();
    const { pathname } = new URL(request.url());
    if (request.method() === "OPTIONS") return route.fulfill({ status: 204, headers: CORS_HEADERS });

    if (pathname.endsWith("/v1/tournaments") && request.method() === "GET") {
      return route.fulfill({ headers: CORS_HEADERS, json: [TOURNAMENT] });
    }
    if (pathname.endsWith(`/v1/tournaments/${TOURNAMENT.id}/register`) && request.method() === "POST") {
      registrations.count += 1;
      if (registrations.count > 1) {
        return route.fulfill({ status: 409, headers: CORS_HEADERS, json: { error: "already registered" } });
      }
      return route.fulfill({
        headers: CORS_HEADERS,
        json: { status: "registered", tournament_id: TOURNAMENT.id },
      });
    }
    return route.fulfill({ status: 404, headers: CORS_HEADERS, json: { error: "not found" } });
  });

  return registrations;
}

test.describe("Tournament registration", () => {
  test("registers for an open tournament", async ({ page }) => {
    const registrations = await mockTournamentApi(page);
    await page.goto("/tournament");

    await expect(page.getByRole("button", { name: /e2e open/i })).toBeVisible();
    await page.getByRole("button", { name: /^register$/i }).click();

    await expect(page.getByText("You're registered for E2E Open.")).toBeVisible();
    await expect(page.getByRole("button", { name: /^registered$/i })).toBeVisible();
    await expect(page.getByText(/already registered/i)).toBeHidden();
    expect(registrations.count).toBe(1);
  });

  test("rejects a duplicate signup", async ({ page }) => {
    const registrations = await mockTournamentApi(page);
    await page.goto("/tournament");

    await page.getByRole("button", { name: /^register$/i }).click();
    await expect(page.getByText("You're registered for E2E Open.")).toBeVisible();

    await page.getByRole("button", { name: /^registered$/i }).click();

    await expect(page.getByText("You are already registered for this tournament.")).toBeVisible();
    await expect(page.getByText("You're registered for E2E Open.")).toBeHidden();
    expect(registrations.count).toBe(2);
  });
});
