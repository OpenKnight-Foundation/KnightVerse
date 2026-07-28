import { test, expect } from "@playwright/test";

test.describe("Landing page", () => {
  test("loads and displays KnightVerse branding", async ({ page }) => {
    await page.goto("/");
    await expect(page).toHaveTitle(/KnightVerse/);
    await expect(
      page.getByRole("region", { name: /home/i }).first(),
    ).toBeVisible();
  });

  test("shows game mode selection", async ({ page }) => {
    await page.goto("/");
    const region = page.getByRole("region", { name: /game mode/i });
    await expect(region).toBeVisible();
    await expect(region.getByRole("button", { name: /play online/i })).toBeVisible();
    await expect(region.getByRole("button", { name: /play vs bot/i })).toBeVisible();
    await expect(region.getByRole("button", { name: /puzzles/i })).toBeVisible();
  });

  test("chessboard renders without crashing", async ({ page }) => {
    await page.goto("/");
    const board = page.getByRole("grid", { name: /chess/i });
    await expect(board).toBeVisible();
    const cells = board.getByRole("gridcell");
    await expect(cells).toHaveCount(64);
  });
});

test.describe("Navigation", () => {
  test("can navigate to dashboard", async ({ page }) => {
    await page.goto("/dashboard");
    await expect(
      page.getByRole("region", { name: /elo progress/i }),
    ).toBeVisible();
  });

  test("can navigate to puzzles", async ({ page }) => {
    await page.goto("/puzzles");
    await expect(page.getByText("Learn-to-Earn Puzzles")).toBeVisible();
  });

  test("can navigate to watch", async ({ page }) => {
    await page.goto("/watch");
    await expect(
      page.getByRole("region", { name: /live games/i }),
    ).toBeVisible();
  });

  test("can navigate to tournaments", async ({ page }) => {
    await page.goto("/tournament");
    await expect(page.getByRole("main", { name: /tournaments/i })).toBeVisible();
  });
});

test.describe("Accessibility", () => {
  test("landing page has skip-to-content link", async ({ page }) => {
    await page.goto("/");
    const skip = page.getByRole("link", { name: /skip to main/i });
    await expect(skip).toBeAttached();
  });

  test("chessboard squares have keyboard support", async ({ page }) => {
    await page.goto("/");
    const board = page.getByRole("grid", { name: /chess/i });
    const firstCell = board.getByRole("gridcell").first();
    await firstCell.focus();
    await expect(firstCell).toBeFocused();
  });
});
