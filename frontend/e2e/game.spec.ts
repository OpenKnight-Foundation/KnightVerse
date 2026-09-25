import { test, expect, type Page } from "@playwright/test";

/**
 * Deterministic stand-in for /assets/stockfish.js. It speaks just enough UCI
 * for useStockfishWASM and answers each "go" with a scripted reply (keyed by
 * the FEN fullmove number) that walks Black into Scholar's Mate.
 */
const SCRIPTED_ENGINE = `
let fen = "";
const replies = { 1: "a7a6", 2: "a6a5", 3: "b7b6" };
onmessage = (event) => {
  const cmd = String(event.data);
  if (cmd === "uci") postMessage("uciok");
  else if (cmd === "isready") postMessage("readyok");
  else if (cmd.startsWith("position fen ")) fen = cmd.slice("position fen ".length);
  else if (cmd.startsWith("go")) {
    const moveNumber = Number(fen.split(" ")[5]);
    postMessage("info depth 1 score cp 0 nodes 1 time 1 pv " + (replies[moveNumber] || ""));
    postMessage("bestmove " + (replies[moveNumber] || "(none)"));
  }
};
`;

function square(page: Page, name: string) {
  return page
    .getByRole("grid", { name: /chess/i })
    .getByRole("gridcell", { name: new RegExp(`^${name}(\\s|$)`) });
}

async function playMove(page: Page, from: string, to: string) {
  // Keyboard selection avoids the fixed "Playing vs Bot" bar intercepting clicks.
  await square(page, from).press("Enter");
  await square(page, to).press("Enter");
  await expect(square(page, to)).toHaveAccessibleName(/ with /);
}

async function expectBotMove(page: Page, to: string) {
  await expect(square(page, to)).toHaveAccessibleName(/ with /, { timeout: 10_000 });
}

test.describe("Full game vs bot", () => {
  test.beforeEach(async ({ page }) => {
    await page.context().route("**/assets/stockfish.js", (route) =>
      route.fulfill({ contentType: "application/javascript", body: SCRIPTED_ENGINE }),
    );
  });

  test("plays from the opening move to checkmate and shows the win overlay", async ({ page }) => {
    await page.goto("/");

    await page.getByRole("button", { name: /play bots/i }).click();
    await page.getByRole("button", { name: /confirm & find match/i }).click();
    await expect(page.getByText("Playing vs Bot")).toBeVisible();
    await expect(page.getByText("Engine ready")).toBeVisible({ timeout: 20_000 });

    await playMove(page, "e2", "e4");
    await expectBotMove(page, "a6");

    await playMove(page, "f1", "c4");
    await expectBotMove(page, "a5");

    await playMove(page, "d1", "h5");
    await expectBotMove(page, "b6");

    // Qxf7# (Scholar's Mate)
    await playMove(page, "h5", "f7");

    await expect(page.getByRole("heading", { name: "You Win!" })).toBeVisible();
    await expect(page.getByText(/checkmate.*excellent play/i)).toBeVisible();

    // Play Again resets the board to the starting position.
    await page.getByRole("button", { name: /play again/i }).click();
    await expect(page.getByRole("heading", { name: "You Win!" })).toBeHidden();
    await expect(square(page, "e2")).toHaveAccessibleName(/ with /);
    await expect(square(page, "f7")).toHaveAccessibleName(/ with /);
    await expect(page.getByText("Make your first move!")).toBeVisible();
  });
});
