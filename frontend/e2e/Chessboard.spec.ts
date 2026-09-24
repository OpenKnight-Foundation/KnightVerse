import { test, expect } from "@playwright/test";

test.describe("Chessboard visual regression", () => {
  test("renders correctly with default theme and piece set", async ({
    page,
  }) => {
    await page.goto("/play/offline");
    await expect(page.locator(".chessboard-container")).toHaveScreenshot(
      "chessboard-default.png",
    );
  });

  test("renders correctly with emerald theme and staunton piece set", async ({
    page,
  }) => {
    await page.goto("/settings");
    await page.selectOption('[data-testid="piece-set-selector"]', "staunton");
    await page.click(
      '[aria-label="Curated Board Theme Presets"] button:has-text("Emerald")',
    );
    await page.goto("/play/offline");
    await expect(page.locator(".chessboard-container")).toHaveScreenshot(
      "chessboard-emerald-staunton.png",
    );
  });

  test("renders correctly with a specific FEN position", async ({ page }) => {
    await page.goto(
      "/play/offline?fen=r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
    );
    await expect(page.locator(".chessboard-container")).toHaveScreenshot(
      "chessboard-specific-fen.png",
    );
  });
});
