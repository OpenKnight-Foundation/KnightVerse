/**
 * Client-side chess scoresheet PDF generation (FE-79)
 *
 * Turns a completed game (PGN-derived metadata + SAN move list) into a
 * printable A4 scoresheet, entirely in the browser with no third-party PDF
 * dependency and no backend round trip.
 *
 * The generator emits a minimal, spec-compliant PDF 1.4 document by hand:
 *
 *   - object graph: catalog + page tree + N pages (each with one content stream)
 *   - fonts: Helvetica (prose) and Courier (move table + board diagram),
 *     both standard Type1 fonts with WinAnsiEncoding
 *   - pagination: when the move list overflows one A4 page, remaining moves
 *     flow onto additional pages
 *   - xref table + trailer so the file is valid for any PDF reader / printer
 *
 * Text is encoded as WinAnsi bytes; characters that cannot be represented
 * (code points > 255, control characters) are lossily replaced so the output
 * is always structurally valid.
 */

// ── Public API ────────────────────────────────────────────────────────────────

export interface ScoresheetMetadata {
  /** White player name. */
  white: string;
  /** Black player name. */
  black: string;
  /** White rating (e.g. "1280"). */
  whiteElo?: string;
  /** Black rating (e.g. "1263"). */
  blackElo?: string;
  /** Game date, usually "YYYY.MM.DD" per PGN convention. */
  date?: string;
  /** Game result symbol, e.g. "1-0", "0-1", "1/2-1/2" or "*". */
  result?: string;
  /** Tournament / match name. */
  event?: string;
  /** Played-at location. */
  site?: string;
  /** Time control, e.g. "300+3". */
  timeControl?: string;
}

export interface ScoresheetInput {
  metadata: ScoresheetMetadata;
  /**
   * SAN move strings in play order (ply order).
   * A complete game produces "e4", "e5", "Nf3", "Nc6", ... and so on.
   */
  moves: string[];
  /**
   * FEN of the final position. When provided, an ASCII board diagram of the
   * final position is included on the scoresheet.
   */
  finalFen?: string;
}

/**
 * Build a scoresheet PDF for the given game.
 * Returns raw PDF bytes ready to be saved as a `Blob`.
 */
export function buildScoresheetPdf(input: ScoresheetInput): Uint8Array {
  const pageLines = paginate(buildRenderLines(input));

  const writer = new ByteWriter();
  const objectOffsets: number[] = [];

  const beginObject = (num: number): void => {
    objectOffsets[num] = writer.length;
    writer.writeString(`${num} 0 obj\n`);
  };
  const endObject = (): void => writer.writeString("endobj\n");

  writer.writeString("%PDF-1.4\n");

  // 1 — catalog
  beginObject(1);
  writer.writeString("<< /Type /Catalog /Pages 2 0 R >>\n");
  endObject();

  // 2 — page tree
  const pageObjectIds = pageLines.map((_, i) => 5 + i * 2);
  beginObject(2);
  writer.writeString(
    `<< /Type /Pages /Kids [${pageObjectIds.map((id) => `${id} 0 R`).join(" ")}] /Count ${pageLines.length} >>\n`,
  );
  endObject();

  // 3 — Helvetica (prose)
  beginObject(3);
  writer.writeString(
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\n",
  );
  endObject();

  // 4 — Courier (move list + board diagram)
  beginObject(4);
  writer.writeString(
    "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>\n",
  );
  endObject();

  // 5.. — one page object + one content stream per page
  pageLines.forEach((lines, index) => {
    const pageObjectId = 5 + index * 2;
    const contentObjectId = pageObjectId + 1;
    const stream = renderContent(lines);

    beginObject(contentObjectId);
    writer.writeString(`<< /Length ${stream.length} >>\nstream\n${stream}\nendstream\n`);
    endObject();

    beginObject(pageObjectId);
    writer.writeString(
      `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${PAGE.WIDTH} ${PAGE.HEIGHT}] ` +
        `/Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents ${contentObjectId} 0 R >>\n`,
    );
    endObject();
  });

  // xref table + trailer
  const totalObjects = 4 + pageLines.length * 2 + 1;
  const xrefOffset = writer.length;
  writer.writeString(`xref\n0 ${totalObjects}\n`);
  writer.writeString("0000000000 65535 f \n");
  for (let num = 1; num < totalObjects; num++) {
    writer.writeString(`${String(objectOffsets[num]).padStart(10, "0")} 00000 n \n`);
  }
  writer.writeString(
    `trailer\n<< /Size ${totalObjects} /Root 1 0 R >>\nstartxref\n${xrefOffset}\n%%EOF\n`,
  );

  return writer.toUint8Array();
}

/**
 * Derive a safe, human-readable filename for a scoresheet download,
 * e.g. `scoresheet-white-vs-black-20260326.pdf`.
 */
export function scoresheetFilename(metadata: ScoresheetMetadata): string {
  const slug = (value?: string): string =>
    (value ?? "")
      .replace(/[^a-z0-9]+/gi, "-")
      .replace(/^-+|-+$/g, "")
      .toLowerCase();
  const white = slug(metadata.white) || "white";
  const black = slug(metadata.black) || "black";
  const date = (metadata.date ?? "").replace(/[^0-9]/g, "");
  return `scoresheet-${white}-vs-${black}${date ? `-${date}` : ""}.pdf`;
}

/**
 * Build and trigger a browser download of the scoresheet for the given game.
 * Creates a temporary anchor element and releases the object URL afterwards.
 */
export function downloadScoresheetPdf(input: ScoresheetInput): void {
  const bytes = buildScoresheetPdf(input);
  const blob = new Blob([bytes], { type: "application/pdf" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = scoresheetFilename(input.metadata);
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

// ── Layout / rendering internals ──────────────────────────────────────────────

/** A4 page in PDF points (1 pt = 1/72 inch). */
const PAGE = { WIDTH: 595, HEIGHT: 842, MARGIN_X: 50, MARGIN_TOP: 60, MARGIN_BOTTOM: 50 };

type FontKey = "F1" | "F2";

interface RenderLine {
  text: string;
  font: FontKey;
  size: number;
}

function lineHeight(size: number): number {
  return Math.round(size * 1.5);
}

/** Compose the full render line list (title, metadata, board, moves). */
function buildRenderLines(input: ScoresheetInput): RenderLine[] {
  const { metadata, moves, finalFen } = input;
  const lines: RenderLine[] = [];
  const push = (text: string, font: FontKey, size: number): void => {
    lines.push({ text, font, size });
  };
  const blank = (size = 4): void => push("", "F1", size);

  const withRating = (name: string, elo?: string): string =>
    elo ? `${name} (${elo})` : name;

  push("Chess Scoresheet", "F1", 18);
  blank();
  blank();

  const metaLines: Array<[string, string | undefined]> = [
    ["Event", metadata.event],
    ["Site", metadata.site],
    ["Date", metadata.date],
    ["White", withRating(metadata.white, metadata.whiteElo)],
    ["Black", withRating(metadata.black, metadata.blackElo)],
    ["Result", metadata.result],
    ["Time control", metadata.timeControl],
  ];
  for (const [label, value] of metaLines) {
    if (value) push(`${label}: ${value}`, "F1", 11);
  }

  if (finalFen) {
    blank();
    push("Final Position", "F1", 12);
    for (const boardLine of buildBoard(finalFen)) push(boardLine, "F2", 11);
  }

  blank();
  push("Moves", "F1", 12);
  for (const row of buildMoveRows(moves)) push(row, "F2", 11);

  return lines;
}

/** Split the render lines into pages, flowing lines top-to-bottom. */
function paginate(lines: RenderLine[]): RenderLine[][] {
  const pages: RenderLine[][] = [];
  let current: RenderLine[] = [];
  let y = PAGE.HEIGHT - PAGE.MARGIN_TOP;

  for (const line of lines) {
    const lh = lineHeight(line.size);
    y -= lh;
    if (y < PAGE.MARGIN_BOTTOM) {
      pages.push(current);
      current = [];
      y = PAGE.HEIGHT - PAGE.MARGIN_TOP - lh;
    }
    current.push(line);
  }
  if (current.length > 0) pages.push(current);

  return pages;
}

/** Render a page's lines into a PDF content stream. */
function renderContent(lines: RenderLine[]): string {
  const parts: string[] = [];
  let y = PAGE.HEIGHT - PAGE.MARGIN_TOP;

  for (const line of lines) {
    y -= lineHeight(line.size);
    if (line.text) {
      parts.push(
        `BT\n/${line.font} ${line.size} Tf\n1 0 0 1 ${PAGE.MARGIN_X} ${y} Tm\n` +
          `(${escapePdfString(line.text)}) Tj\nET`,
      );
    }
  }
  return parts.join("\n");
}

/** Format SAN moves as aligned `NN. White Black` scoresheet rows. */
function buildMoveRows(moves: string[]): string[] {
  if (moves.length === 0) return ["No moves recorded"];
  const rows: string[] = [];
  for (let i = 0; i < moves.length; i += 2) {
    const moveNumber = i / 2 + 1;
    const white = moves[i] ?? "";
    const black = moves[i + 1] ?? "";
    rows.push(`${String(moveNumber).padStart(4)}. ${white.padEnd(10)}${black}`);
  }
  return rows;
}

/** Render an ASCII board diagram from a FEN position (uppercase = white). */
function buildBoard(fen: string): string[] {
  const placement = fen.split(" ")[0] ?? "";
  const lines = ["  +-----------------+"];
  for (const rank of placement.split("/").filter(Boolean)) {
    const cells: string[] = [];
    for (const char of rank) {
      const empty = char.match(/[1-8]/);
      if (empty) {
        for (let i = 0; i < Number(char); i++) cells.push(".");
      } else {
        cells.push(char);
      }
    }
    lines.push(`  | ${cells.join(" ")} |`);
  }
  lines.push("  +-----------------+");
  return lines;
}

/** Escape text for a PDF literal string (parentheses, backslash, control chars). */
function escapePdfString(value: string): string {
  let out = "";
  for (const char of value) {
    const code = char.codePointAt(0) ?? 0;
    if (code === 0x28) out += "\\(";
    else if (code === 0x29) out += "\\)";
    else if (code === 0x5c) out += "\\\\";
    else if (code < 32 || code === 0x7f) out += " ";
    else out += char;
  }
  return out;
}

/** Byte accumulator used to emit the PDF with exact xref offsets. */
class ByteWriter {
  private bytes: number[] = [];

  get length(): number {
    return this.bytes.length;
  }

  /** Append a string as WinAnsi-style bytes (code points > 255 become "?"). */
  writeString(value: string): void {
    for (let i = 0; i < value.length; i++) {
      const code = value.charCodeAt(i);
      this.bytes.push(code <= 255 ? code : 0x3f);
    }
  }

  toUint8Array(): Uint8Array {
    return Uint8Array.from(this.bytes);
  }
}