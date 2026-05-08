import type { SpectatorMove } from "@/hook/useSpectatorSocket";
import {
  formatClock,
  getCapturedFromMoves,
  getGamePhase,
  truncateAddress,
} from "@/lib/spectatorUtils";

describe("spectatorUtils", () => {
  describe("formatClock", () => {
    it("formats seconds into mm:ss", () => {
      expect(formatClock(0)).toBe("0:00");
      expect(formatClock(60)).toBe("1:00");
      expect(formatClock(599)).toBe("9:59");
      expect(formatClock(3600)).toBe("60:00");
    });
  });

  describe("truncateAddress", () => {
    it("truncates stellar addresses", () => {
      expect(truncateAddress("GABCDEFGHIJKLMNOP")).toBe("GABCDE...LMNOP");
    });

    it("returns short addresses unchanged", () => {
      expect(truncateAddress("GSHORT123")).toBe("GSHORT123");
    });
  });

  describe("getCapturedFromMoves", () => {
    it("tracks captured pieces for both colors", () => {
      const moves: SpectatorMove[] = [
        { from: "e2", to: "e4", san: "e4", color: "w" },
        { from: "d7", to: "d5", san: "d5", color: "b" },
        { from: "e4", to: "d5", san: "exd5", color: "w" },
        { from: "d8", to: "d5", san: "Qxd5", color: "b" },
        { from: "b1", to: "c3", san: "Nc3", color: "w" },
      ];

      expect(getCapturedFromMoves(moves)).toEqual({
        white: ["p"],
        black: ["p"],
      });
    });

    it("handles empty move lists", () => {
      expect(getCapturedFromMoves([])).toEqual({ white: [], black: [] });
    });
  });

  describe("getGamePhase", () => {
    it("returns the correct phase by move count", () => {
      expect(getGamePhase(0)).toBe("opening");
      expect(getGamePhase(10)).toBe("opening");
      expect(getGamePhase(11)).toBe("middlegame");
      expect(getGamePhase(30)).toBe("middlegame");
      expect(getGamePhase(31)).toBe("endgame");
    });
  });
});
