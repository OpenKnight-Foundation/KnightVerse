import { test, expect, type Page } from "@playwright/test";

const PUZZLE = {
  id: 7,
  fen: "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4",
  title: "Fork Attack",
  difficulty: "easy",
  description: "Find the knight fork that wins material",
  solution: [{ from: "f3", to: "g5" }],
  hint: "Look for a knight move that attacks two pieces",
};

const CORS_HEADERS = {
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Headers": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
};

const SUBMIT_ROUTE = /\/v1\/puzzles\/(?:[^/]+\/)?submit(?:\?|$)/;

async function setup(page: Page, verified: boolean): Promise<void> {
  await page.route("**/v1/puzzles", (route) => {
    if (route.request().method() === "OPTIONS") {
      return route.fulfill({ status: 204, headers: CORS_HEADERS });
    }
    return route.fulfill({
      headers: CORS_HEADERS,
      json: { puzzles: [PUZZLE] },
    });
  });

  await page.route(SUBMIT_ROUTE, (route) => {
    if (route.request().method() === "OPTIONS") {
      return route.fulfill({ status: 204, headers: CORS_HEADERS });
    }
    return route.fulfill({
      headers: CORS_HEADERS,
      json: { verified, reward: verified ? 5 : 0 },
    });
  });
}

async function playMove(page: Page, from: string, to: string): Promise<void> {
  const squareSelector = (square: string) =>
    `[data-square="${8 - Number(square[1])}-${square.charCodeAt(0) - 97}"]`;
  await page.locator(squareSelector(from)).click();
  await page.locator(squareSelector(to)).click();
}

async function openPuzzle(page: Page): Promise<void> {
  await page.goto("/puzzles");
  await page.getByRole("button", { name: /Fork Attack/ }).click();
  await page.locator('[data-square="5-5"]').waitFor();
  await playMove(page, "f3", "g5");
  const submit = page.getByRole("button", { name: /Submit Solution/ });
  await expect(submit).toBeVisible();
  await expect(submit).toBeEnabled();
}

test.describe("solving a puzzle", () => {
  test("accepts a correct solution and shows the verified reward", async ({ page }) => {
    await setup(page, true);
    await openPuzzle(page);

    const submitted = page.waitForRequest(
      (request) =>
        SUBMIT_ROUTE.test(request.url()) && request.method() === "POST",
    );
    await page.getByRole("button", { name: /Submit Solution/ }).click();
    const request = await submitted;

    expect(request.postDataJSON()).toEqual({
      puzzle_id: PUZZLE.id,
      moves: ["f3g5"],
    });
    await expect(page.getByText(/Correct! Well done!/)).toBeVisible();
    const reward = page.getByRole("dialog", { name: /Puzzle reward/ });
    await expect(reward).toBeVisible();
    await expect(reward).toContainText("+5 XLM");
  });

  test("rejects an incorrect solution without granting a reward", async ({ page }) => {
    await setup(page, false);
    await openPuzzle(page);

    const submitted = page.waitForRequest(
      (request) =>
        SUBMIT_ROUTE.test(request.url()) && request.method() === "POST",
    );
    await page.getByRole("button", { name: /Submit Solution/ }).click();
    await submitted;

    await expect(page.getByText(/Not quite right/)).toBeVisible();
    await expect(page.getByRole("dialog", { name: /Puzzle reward/ })).toHaveCount(0);
  });
});
