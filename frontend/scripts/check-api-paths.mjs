import { readdir, readFile } from "node:fs/promises";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const frontendRoot = resolve(here, "..");
const specPath = resolve(frontendRoot, "../openapi.json");
const spec = JSON.parse(await readFile(specPath, "utf8"));

const normalize = (path) => path.replace(/\/\{[^}]*\}/g, "/*").replace(/\{[^}]*\}/g, "");
const knownPaths = new Set(Object.keys(spec.paths ?? {}).map(normalize));

// Endpoints the frontend already calls behind a preview fallback, before the
// backend ships them. Remove an entry once the route is in openapi.json.
const plannedPaths = [
  "/v1/guilds/*", // guildService.fetchGuild falls back to PREVIEW_FIXTURES
  "/v1/games/*/win-claim", // useClaimWinnings reports 404/501 as "not available yet"
];
for (const path of plannedPaths) knownPaths.add(path);
const skippedDirectories = new Set(["node_modules", ".next", "dist", "coverage", "__tests__", "e2e"]);
const violations = [];
let checked = 0;

async function walk(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (skippedDirectories.has(entry.name)) continue;
    const fullPath = resolve(directory, entry.name);
    if (entry.isDirectory()) {
      await walk(fullPath);
      continue;
    }
    if (!/\.(ts|tsx)$/.test(entry.name) || fullPath.endsWith("lib/api/generated.ts")) continue;

    checked++;
    const source = (await readFile(fullPath, "utf8"))
      .replace(/\/\*[\s\S]*?\*\//g, " ")
      .replace(/\/\/[^\n]*/g, " ")
      .replace(/\$\{[^}]*\}/g, "{param}");
    const used = source.match(/\/v1\/[A-Za-z0-9_{}\-.\/]+/g) ?? [];
    for (const raw of used) {
      const path = normalize(raw.replace(/\/+$/, ""));
      if (!knownPaths.has(path)) {
        violations.push(`${relative(frontendRoot, fullPath)}: ${path}`);
      }
    }
  }
}

await walk(frontendRoot);

if (violations.length > 0) {
  console.error(`Found /v1 paths missing from openapi.json:\n${[...new Set(violations)].join("\n")}`);
  process.exit(1);
}

console.log(`Checked ${checked} frontend files against ${knownPaths.size} spec paths.`);
