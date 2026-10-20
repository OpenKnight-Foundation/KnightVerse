#!/usr/bin/env node
/**
 * Build-time check for #FE-86: proves NEXT_PUBLIC_API_URL is inlined into the
 * client bundle.
 *
 *   NEXT_PUBLIC_API_URL=https://env-inlining-check.invalid npm run build
 *   node scripts/check-env-inlining.mjs
 *
 * Exits non-zero when the sentinel URL is missing from .next/static.
 */
import { readdirSync, readFileSync, statSync } from "fs";
import path from "path";

const SENTINEL = process.env.NEXT_PUBLIC_API_URL ?? "https://env-inlining-check.invalid";
const STATIC_DIR = path.resolve(import.meta.dirname, "..", ".next", "static");

function* files(dir) {
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    if (statSync(full).isDirectory()) yield* files(full);
    else if (full.endsWith(".js")) yield full;
  }
}

let found = null;
try {
  for (const file of files(STATIC_DIR)) {
    if (readFileSync(file, "utf8").includes(SENTINEL)) {
      found = path.relative(STATIC_DIR, file);
      break;
    }
  }
} catch (err) {
  console.error(`Could not read ${STATIC_DIR}. Run \`npm run build\` first.\n${err.message}`);
  process.exit(2);
}

if (!found) {
  console.error(`NEXT_PUBLIC_API_URL (${SENTINEL}) was not inlined into any client chunk.`);
  process.exit(1);
}
console.log(`OK: NEXT_PUBLIC_API_URL inlined into .next/static/${found}`);
