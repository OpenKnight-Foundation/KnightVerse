import { describe, expect, it } from "vitest";

import {
  PIP_DEFAULT_WIDTH,
  PIP_EDGE_SNAP_THRESHOLD,
  PIP_MAX_WIDTH,
  PIP_MIN_WIDTH,
  PIP_STACK_OFFSET,
  PIP_VIEWPORT_MARGIN,
  bringToFront,
  clampRectToViewport,
  closePipWindow,
  defaultPipRect,
  evaluationPercent,
  fenToBoardRows,
  formatEvaluation,
  isPointInRect,
  materialBalanceCp,
  moveNotificationText,
  movePipWindow,
  nextStackSlot,
  openPipWindow,
  resizePipWindow,
  snapRectToEdges,
  topZIndex,
  updatePipRect,
  type PipWindowState,
} from "@/lib/pipWindow";

const VIEWPORT = { width: 1280, height: 800 };

function windowState(gameId: string, overrides: Partial<PipWindowState> = {}): PipWindowState {
  return {
    id: `pip-${gameId}`,
    gameId,
    rect: { x: 100, y: 100, width: PIP_DEFAULT_WIDTH, height: PIP_DEFAULT_WIDTH },
    zIndex: 1,
    ...overrides,
  };
}

describe("clampRectToViewport", () => {
  it("keeps a rect inside the viewport with a margin", () => {
    const clamped = clampRectToViewport({ x: 5000, y: 5000, width: 200, height: 200 }, VIEWPORT);
    expect(clamped.x).toBe(VIEWPORT.width - 200 - PIP_VIEWPORT_MARGIN);
    expect(clamped.y).toBe(VIEWPORT.height - 200 - PIP_VIEWPORT_MARGIN);
  });

  it("pins to the top-left margin when the viewport is smaller than the window", () => {
    const clamped = clampRectToViewport({ x: -50, y: -20, width: 400, height: 400 }, { width: 300, height: 300 });
    expect(clamped.x).toBe(PIP_VIEWPORT_MARGIN);
    expect(clamped.y).toBe(PIP_VIEWPORT_MARGIN);
  });
});

describe("snapRectToEdges", () => {
  it("snaps to the left and top edges within the threshold", () => {
    const snapped = snapRectToEdges({ x: PIP_VIEWPORT_MARGIN + 5, y: PIP_VIEWPORT_MARGIN + 4, width: 200, height: 200 }, VIEWPORT);
    expect(snapped.x).toBe(PIP_VIEWPORT_MARGIN);
    expect(snapped.y).toBe(PIP_VIEWPORT_MARGIN);
  });

  it("snaps to the right and bottom edges within the threshold", () => {
    const right = VIEWPORT.width - 200 - PIP_VIEWPORT_MARGIN - 6;
    const bottom = VIEWPORT.height - 200 - PIP_VIEWPORT_MARGIN - 6;
    const snapped = snapRectToEdges({ x: right, y: bottom, width: 200, height: 200 }, VIEWPORT);
    expect(snapped.x).toBe(VIEWPORT.width - 200 - PIP_VIEWPORT_MARGIN);
    expect(snapped.y).toBe(VIEWPORT.height - 200 - PIP_VIEWPORT_MARGIN);
  });

  it("leaves edges alone beyond the snap threshold", () => {
    const x = PIP_VIEWPORT_MARGIN + PIP_EDGE_SNAP_THRESHOLD + 40;
    const snapped = snapRectToEdges({ x, y: x, width: 200, height: 200 }, VIEWPORT);
    expect(snapped.x).toBe(x);
    expect(snapped.y).toBe(x);
  });
});

describe("movePipWindow", () => {
  it("applies the pointer delta", () => {
    const moved = movePipWindow({ x: 300, y: 300, width: 200, height: 200 }, 50, -40, VIEWPORT);
    expect(moved.x).toBe(350);
    expect(moved.y).toBe(260);
  });

  it("clamps a drag past the viewport edge", () => {
    const moved = movePipWindow({ x: 1000, y: 600, width: 200, height: 200 }, 5000, 5000, VIEWPORT);
    expect(moved.x).toBe(VIEWPORT.width - 200 - PIP_VIEWPORT_MARGIN);
    expect(moved.y).toBe(VIEWPORT.height - 200 - PIP_VIEWPORT_MARGIN);
  });
});

describe("resizePipWindow", () => {
  it("clamps width to the accepted 180–400px range", () => {
    const tooSmall = resizePipWindow({ x: 10, y: 10, width: 240, height: 240 }, 90, VIEWPORT);
    expect(tooSmall.width).toBe(PIP_MIN_WIDTH);
    const tooLarge = resizePipWindow({ x: 10, y: 10, width: 240, height: 240 }, 900, VIEWPORT);
    expect(tooLarge.width).toBe(PIP_MAX_WIDTH);
  });

  it("keeps the board square and the whole rect on-screen", () => {
    const resized = resizePipWindow({ x: 1200, y: 700, width: 240, height: 240 }, 400, VIEWPORT);
    expect(resized.width).toBe(400);
    expect(resized.height).toBe(400);
    expect(resized.x).toBeLessThanOrEqual(VIEWPORT.width - 400 - PIP_VIEWPORT_MARGIN);
    expect(resized.y).toBeLessThanOrEqual(VIEWPORT.height - 400 - PIP_VIEWPORT_MARGIN);
  });
});

describe("window stacking", () => {
  it("places the first window in the bottom-right corner", () => {
    const rect = defaultPipRect(VIEWPORT);
    expect(rect.width).toBe(PIP_DEFAULT_WIDTH);
    expect(rect.x).toBe(VIEWPORT.width - PIP_DEFAULT_WIDTH - PIP_VIEWPORT_MARGIN);
    expect(rect.y).toBe(VIEWPORT.height - PIP_DEFAULT_WIDTH - PIP_VIEWPORT_MARGIN);
  });

  it("offsets each subsequent stack slot", () => {
    const second = nextStackSlot([windowState("a")], VIEWPORT);
    const first = defaultPipRect(VIEWPORT);
    expect(second.x).toBe(first.x - PIP_STACK_OFFSET);
    expect(second.y).toBe(first.y - PIP_STACK_OFFSET);
  });

  it("opens windows with increasing z-index and never duplicates a game", () => {
    let windows = openPipWindow([], "game-1", VIEWPORT);
    windows = openPipWindow(windows, "game-2", VIEWPORT);
    expect(windows).toHaveLength(2);
    expect(topZIndex(windows)).toBe(2);

    windows = openPipWindow(windows, "game-1", VIEWPORT);
    expect(windows).toHaveLength(2);
    expect(windows.find((w) => w.gameId === "game-1")?.zIndex).toBe(3);
  });

  it("raises, closes and updates windows", () => {
    let windows = [windowState("a", { zIndex: 1 }), windowState("b", { zIndex: 2 })];
    windows = bringToFront(windows, "a");
    expect(windows.find((w) => w.gameId === "a")?.zIndex).toBe(3);

    windows = updatePipRect(windows, "b", { x: 5, y: 5, width: 200, height: 200 });
    expect(windows.find((w) => w.gameId === "b")?.rect.x).toBe(5);

    windows = closePipWindow(windows, "a");
    expect(windows.map((w) => w.gameId)).toEqual(["b"]);
  });

  it("detects whether a point is inside a window", () => {
    const rect = { x: 10, y: 10, width: 100, height: 100 };
    expect(isPointInRect(50, 50, rect)).toBe(true);
    expect(isPointInRect(5, 50, rect)).toBe(false);
  });
});

describe("board presentation", () => {
  it("expands the starting position into 8x8 glyphs", () => {
    const rows = fenToBoardRows("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    expect(rows).toHaveLength(8);
    expect(rows[0]).toEqual(["♜", "♞", "♝", "♛", "♚", "♝", "♞", "♜"]);
    expect(rows[7]).toEqual(["♖", "♘", "♗", "♕", "♔", "♗", "♘", "♖"]);
    expect(rows[3].every((glyph) => glyph === "")).toBe(true);
  });

  it("degrades to an empty board for malformed input", () => {
    expect(fenToBoardRows("not-a-fen")).toHaveLength(8);
    expect(fenToBoardRows("not-a-fen")[0]).toHaveLength(8);
    expect(fenToBoardRows(undefined)[0].every((glyph) => glyph === "")).toBe(true);
  });

  it("returns a balanced material count for the starting position", () => {
    expect(materialBalanceCp("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")).toBe(0);
  });

  it("scores a missing black queen as +9 for white", () => {
    expect(materialBalanceCp("rnb1kbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")).toBe(900);
  });

  it("maps centipawns onto a 0–100 evaluation bar", () => {
    expect(evaluationPercent(0)).toBe(50);
    expect(evaluationPercent(1000)).toBe(100);
    expect(evaluationPercent(-1000)).toBe(0);
    expect(evaluationPercent(5000)).toBe(100);
  });

  it("formats evaluations for display", () => {
    expect(formatEvaluation(120)).toBe("+1.2");
    expect(formatEvaluation(-50)).toBe("-0.5");
    expect(formatEvaluation(0)).toBe("0.0");
    expect(formatEvaluation(10000)).toBe("#");
  });

  it("labels moves with the correct move number and colour ellipsis", () => {
    const white = { from: "e2", to: "e4", san: "e4", color: "w" as const };
    const black = { from: "d7", to: "d5", san: "d5", color: "b" as const };
    expect(moveNotificationText(white, 0)).toBe("1.e4");
    expect(moveNotificationText(black, 1)).toBe("1...d5");
    expect(moveNotificationText(white, 2)).toBe("2.e4");
  });
});
