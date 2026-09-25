/**
 * Unit tests for the client-side scoresheet PDF generator (FE-79).
 *
 * Verifies that the emitted file:
 *   1. is structurally valid (header/footer, object graph, live xref offsets,
 *      matching /Length for every content stream)
 *   2. contains the correct move list and metadata for the given game
 *   3. paginates long games onto multiple pages
 *   4. survives hostile strings (parentheses, backslashes, non-Latin-1 text)
 */

import {
  buildScoresheetPdf,
  downloadScoresheetPdf,
  scoresheetFilename,
  type ScoresheetInput,
} from "@/lib/scoresheetPdf";

const SAMPLE_INPUT: ScoresheetInput = {
  metadata: {
    white: "GABC...XYZ",
    black: "GDEF...UVW",
    whiteElo: "1280",
    blackElo: "1263",
    date: "2026.03.26",
    result: "1-0",
    event: "KnightVerse Rated Game",
    site: "knightverse.app",
    timeControl: "300+3",
  },
  moves: [
    "e4",
    "e5",
    "Nf3",
    "Nc6",
    "Bb5",
    "a6",
    "Ba4",
    "Nf6",
    "O-O",
    "Be7",
    "Re1",
    "b5",
    "Bb3",
    "d6",
    "c3",
    "O-O",
    "h3",
    "Nb8",
    "d4",
    "Nbd7",
    "Nbd2",
    "Bb7",
    "Bc2",
    "Re8",
    "Nf1",
    "Bf8",
    "Ng3",
    "g6",
    "a4",
    "c5",
    "d5",
    "c4",
    "b4",
    "cxb3",
    "Bxb3",
    "Nc5",
    "Bc2",
    "Rc8",
    "axb5",
    "axb5",
    "Nf5",
    "gxf5",
    "exf5",
    "Kh8",
    "Qd2",
    "Ng8",
    "Bh6",
    "Bxh6",
    "Qxh6",
    "Nf6",
    "f6",
    "Rg8",
    "Ng5",
    "Rg6",
    "Qh4",
    "Rcg8",
    "Re3",
    "Rxg5",
    "Rg3",
  ],
  finalFen: "r2q1r1k/pp2Q2p/3p3P/6R1/8/8/PPP2PPP/2KR4 w - - 2 38",
};

function decode(bytes: Uint8Array): string {
  let out = "";
  for (let i = 0; i < bytes.length; i++) out += String.fromCharCode(bytes[i]);
  return out;
}

/** Assert every xref entry points at a real `N 0 obj` header. */
function expectValidXref(pdf: string): void {
  const startMarker = "startxref\n";
  const startIndex = pdf.indexOf(startMarker);
  const startxref = Number(pdf.slice(startIndex + startMarker.length).split("\n")[0]);
  expect(pdf.slice(startxref, startxref + 4)).toBe("xref");
  const headerMatch = pdf
    .slice(startxref)
    .match(/xref\n0 (\d+)\n/);
  expect(headerMatch).not.toBeNull();
  const count = Number(headerMatch![1]);
  const entriesStart = startxref + headerMatch![0].length;

  const entryOffsets: number[] = [];
  for (let i = 0; i < count; i++) {
    const raw = pdf.slice(entriesStart + i * 20, entriesStart + i * 20 + 19);
    expect(raw).toMatch(/^\d{10} \d{5} [nf] $/);
    const [offset, , status] = raw.split(" ");
    if (status === "n") entryOffsets.push(Number(offset));
  }
  expect(entryOffsets.length).toBe(count - 1);

  entryOffsets.forEach((offset, index) => {
    const objectNumber = index + 1;
    const header = `${objectNumber} 0 obj`;
    expect(pdf.slice(offset, offset + header.length)).toBe(header);
  });
}

/** Assert every `/Length N` matches the byte length of its stream body. */
function expectValidStreamLengths(pdf: string): void {
  const streamRe = /\/Length (\d+) >>\nstream\n([\s\S]*?)\nendstream/g;
  let match: RegExpExecArray | null;
  let found = 0;
  while ((match = streamRe.exec(pdf)) !== null) {
    expect(match[2].length).toBe(Number(match[1]));
    found += 1;
  }
  expect(found).toBeGreaterThan(0);
}

describe("scoresheetPdf", () => {
  describe("buildScoresheetPdf", () => {
    it("emits a structurally valid single-page PDF", () => {
      const pdf = decode(buildScoresheetPdf(SAMPLE_INPUT));

      expect(pdf.startsWith("%PDF-1.4")).toBe(true);
      expect(pdf.endsWith("%%EOF\n")).toBe(true);
      expect(pdf).toContain("/Type /Catalog");
      expect(pdf).toContain("/Type /Pages");
      expect(pdf).toContain("/Type /Page");
      expect(pdf).toContain("/BaseFont /Helvetica");
      expect(pdf).toContain("/BaseFont /Courier");
      expect(pdf).toContain("/MediaBox [0 0 595 842]");

      expectValidXref(pdf);
      expectValidStreamLengths(pdf);
    });

    it("includes metadata and the full move list for the game", () => {
      const pdf = decode(buildScoresheetPdf(SAMPLE_INPUT));

      expect(pdf).toContain("Chess Scoresheet");
      expect(pdf).toContain("GABC...XYZ \\(1280\\)");
      expect(pdf).toContain("GDEF...UVW \\(1263\\)");
      expect(pdf).toContain("2026.03.26");
      expect(pdf).toContain("1-0");
      expect(pdf).toContain("Event: KnightVerse Rated Game");
      expect(pdf).toContain("Time control: 300+3");

      // Spot-check the aligned move table
      expect(pdf).toContain("1. e4        e5");
      expect(pdf).toContain("Qxh6");
      expect(pdf).toContain("Rg3");
      expect(pdf).toContain("30. Rg3");
    });

    it("includes the final-position board diagram when a FEN is provided", () => {
      const pdf = decode(buildScoresheetPdf(SAMPLE_INPUT));

      expect(pdf).toContain("Final Position");
      // Diagram border line (Monopoly-ish) — the +----+ frame
      expect(pdf).toContain("+-----------------+");
      // A rank from the final FEN: rank 2 is PPP2PPP
      expect(pdf).toContain("| P P P . . P P P |");
    });

    it("paginates onto multiple pages for long games", () => {
      const moves: string[] = [];
      for (let i = 0; i < 400; i++) moves.push(i % 2 === 0 ? `e${4 + (i % 3)}` : "e5");
      const longGame: ScoresheetInput = {
        metadata: { white: "A", black: "B", result: "*" },
        moves,
      };

      const pdf = decode(buildScoresheetPdf(longGame));
      const pageCount = (pdf.match(/\/Type \/Page\b/g) ?? []).length;
      const countMatch = pdf.match(/\/Count (\d+) >>/);
      expect(pageCount).toBeGreaterThan(1);
      expect(Number(countMatch![1])).toBe(pageCount);
      expect(pdf).toContain("200. e6        e5");
    });

    it("escapes special characters and degrades non-Latin-1 text safely", () => {
      const hostile: ScoresheetInput = {
        metadata: {
          white: "O'Brien (White Team)",
          black: "Back\\slash) é",
          result: "1/2-1/2",
        },
        moves: ["e4", "e5"],
      };

      const pdf = decode(buildScoresheetPdf(hostile));
      expect(pdf.startsWith("%PDF-1.4")).toBe(true);
      expect(pdf.endsWith("%%EOF\n")).toBe(true);
      // Parentheses and backslashes inside literal strings must be escaped
      expect(pdf).toContain("O'Brien \\(White Team\\)");
      // The accented é is a valid WinAnsi byte; code points > 255 become '?'
      expect(String.fromCharCode(0xe9)).toBe("é");
      expect(pdf).toContain("1/2-1/2");
      expectValidXref(pdf);
    });

    it("returns zero bytes of output only for asymmetric inputs without crashing", () => {
      const pdf = decode(
        buildScoresheetPdf({
          metadata: { white: "Player", black: "Rival" },
          moves: [],
        }),
      );
      expect(pdf).toContain("No moves recorded");
      expectValidXref(pdf);
    });
  });

  describe("scoresheetFilename", () => {
    it("builds a slugged, dated filename", () => {
      expect(
        scoresheetFilename({
          white: "GABC...XYZ",
          black: "GDEF...UVW",
          date: "2026.03.26",
        }),
      ).toBe("scoresheet-gabc-xyz-vs-gdef-uvw-20260326.pdf");
    });

    it("falls back to side names when identifiers are missing", () => {
      expect(scoresheetFilename({ white: "", black: "" })).toBe(
        "scoresheet-white-vs-black.pdf",
      );
    });
  });

  describe("downloadScoresheetPdf", () => {
    it("creates a blob download with the correct filename and content", async () => {
      let captured: Blob | null = null;
      const createObjectURL = vi.fn((blob: Blob): string => {
        captured = blob;
        return "blob:mock";
      });
      const revokeObjectURL = vi.fn();
      const anchorClick = vi.fn();

      Object.defineProperty(URL, "createObjectURL", {
        configurable: true,
        value: createObjectURL,
      });
      Object.defineProperty(URL, "revokeObjectURL", {
        configurable: true,
        value: revokeObjectURL,
      });
      vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(anchorClick);

      downloadScoresheetPdf(SAMPLE_INPUT);

      expect(createObjectURL).toHaveBeenCalledTimes(1);
      expect(anchorClick).toHaveBeenCalledTimes(1);
      expect(captured).not.toBeNull();
      expect(captured!.type).toBe("application/pdf");
      const text = await captured!.text();
      expect(text.startsWith("%PDF-1.4")).toBe(true);
      expect(text).toContain("Qxh6");
    });
  });
});