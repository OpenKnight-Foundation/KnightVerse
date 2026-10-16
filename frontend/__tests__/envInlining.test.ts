import { readdirSync, readFileSync, statSync } from "fs";
import path from "path";
import { describe, expect, it } from "vitest";

const ROOT = path.resolve(__dirname, "..");
const SKIP_DIRS = new Set(["node_modules", ".next", "e2e", "__tests__", "public", "scripts"]);

function sourceFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const full = path.join(dir, entry);
    if (statSync(full).isDirectory()) return SKIP_DIRS.has(entry) ? [] : sourceFiles(full);
    return /\.(ts|tsx)$/.test(entry) && !/\.test\.tsx?$/.test(entry) ? [full] : [];
  });
}

describe("NEXT_PUBLIC_* inlining (#FE-86)", () => {
  it("never reads process.env with a computed key in app code", () => {
    // Next.js only inlines `process.env.NEXT_PUBLIC_X` written literally;
    // `process.env[name]` is always undefined in the browser bundle.
    const offenders = sourceFiles(ROOT)
      .filter((file) => /process\.env\s*\[/.test(readFileSync(file, "utf8")))
      .map((file) => path.relative(ROOT, file));

    expect(offenders).toEqual([]);
  });

  it("references every lib/api.ts variable literally", () => {
    const api = readFileSync(path.join(ROOT, "lib/api.ts"), "utf8");

    for (const name of [
      "NEXT_PUBLIC_API_URL",
      "NEXT_PUBLIC_BACKEND_URL",
      "NEXT_PUBLIC_HORIZON_URL",
      "NEXT_PUBLIC_SOROBAN_RPC",
      "NEXT_PUBLIC_NETWORK_PASSPHRASE",
      "NEXT_PUBLIC_IPFS_GATEWAY",
    ]) {
      expect(api).toContain(`process.env.${name}`);
    }
  });
});
