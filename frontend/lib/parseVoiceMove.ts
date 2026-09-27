"use client";

/**
 * FE-52: parse spoken English chess moves into SAN.
 * Handles e.g. "Knight f3" -> "Nf3", "Pawn to e4" -> "e4",
 * "Queen takes d7" -> "Qxd7", "castle kingside" -> "O-O".
 */

const NUMBER_WORDS: Record<string, string> = {
  one: "1",
  two: "2",
  three: "3",
  four: "4",
  five: "5",
  six: "6",
  seven: "7",
  eight: "8",
};

const PIECE_WORDS: Array<[RegExp, string]> = [
  [/\bking\b/, "K"],
  [/\bqueen\b/, "Q"],
  [/\brook\b/, "R"],
  [/\bbishop\b/, "B"],
  [/\bknight\b/, "N"],
  [/\bpawn\b/, ""],
];

function normalizeNumbers(input: string): string {
  let out = ` ${input} `;
  for (const [word, digit] of Object.entries(NUMBER_WORDS)) {
    out = out.replace(new RegExp(`\\b${word}\\b`, "g"), digit);
  }
  // "e four" -> "e4", "a 8" -> "a8"
  out = out.replace(/\b([a-h])\s+([1-8])\b/g, "$1$2");
  return out;
}

export function parseVoiceToSan(rawTranscript: string): string | null {
  if (!rawTranscript) return null;
  let text = rawTranscript.toLowerCase().trim();

  // Castling first (before generic normalization strips meaning)
  if (/castle.*kingside|castles.*kingside|king.*side.*castle|short.*castle|\bo\s*-\s*o\b/.test(text) && !/queen/.test(text)) {
    // disambiguate "o-o" spoken; default kingside
    if (/\bqueen\b/.test(text)) return "O-O-O";
    return "O-O";
  }
  if (/castle.*queenside|castles.*queenside|long.*castle|queen.*side.*castle|\bo\s*-\s*o\s*-\s*o\b/.test(text)) {
    return "O-O-O";
  }
  if (/^\s*(castles|castle)\s*$/.test(text)) return "O-O";

  text = normalizeNumbers(text);

  // Remove filler words
  text = text
    .replace(/\bplease\b/g, "")
    .replace(/\bmove\b/g, "")
    .replace(/\bpiece\b/g, "")
    .replace(/\bto\b/g, " ")
    .replace(/\btakes?\b/g, "x")
    .replace(/\bcaptures?\b/g, "x")
    .replace(/\bx\b/g, "x")
    .replace(/\btake\b/g, "x")
    .replace(/\bcheck\s*mate\b/g, "#")
    .replace(/\bcheck\b/g, "+")
    .replace(/\bmate\b/g, "#")
    .replace(/\s+/g, " ")
    .trim();

  // Detect piece letter
  let piece = "";
  for (const [re, san] of PIECE_WORDS) {
    if (re.test(text)) {
      piece = san;
      text = text.replace(re, " ");
      break;
    }
  }
  // Single-letter piece abbreviations ("n f 3", "q d 7")
  const abbrev = text.match(/\b([kqrbnp])\b\s*([a-h]?\s*x?\s*[a-h][1-8]|[a-h][1-8])/);
  if (abbrev && !piece) {
    piece = abbrev[1] === "p" ? "" : abbrev[1].toUpperCase();
    text = text.replace(abbrev[0], ` ${abbrev[2]} `);
  }

  text = text.replace(/\s+/g, "").trim();

  // Already SAN-like ("nf3", "qxd7", "e4", "o-o")
  const sanLike = text.match(/^([kqrbnp]?)([a-h]?[1-8]?)(x?)([a-h][1-8])([+#]?)(=(q|r|b|n))?$/i);
  if (sanLike) {
    let [, p, disamb, cap, sq, suffix, promo] = sanLike;
    p = piece || (p ? p.toUpperCase() : "");
    // Pawn moves must not carry a piece letter
    if (p === "P") p = "";
    // Pawns capturing need file: "exd5" — if capture with no file, invalid
    if (p === "" && cap === "x" && !disamb) return null;
    return `${p}${disamb}${cap}${sq}${suffix}${promo ?? ""}`;
  }

  // Bare square ("e4") -> pawn push
  if (/^[a-h][1-8][+#]?$/.test(text)) return text;

  return null;
}

export default parseVoiceToSan;
