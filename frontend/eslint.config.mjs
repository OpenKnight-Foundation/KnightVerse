import { dirname } from "path";
import { fileURLToPath } from "url";
import { FlatCompat } from "@eslint/eslintrc";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const compat = new FlatCompat({
  baseDirectory: __dirname,
});

const eslintConfig = [
  ...compat.extends("next/core-web-vitals", "next/typescript"),
  {
    rules: {
      // FE-130: debug traces must go through `lib/debugLogger` (a no-op in
      // production) instead of `console.log`. `warn`/`error` still report real
      // problems, so they stay allowed.
      "no-console": ["error", { allow: ["warn", "error"] }],
    },
  },
  {
    // Tests legitimately assert on console output and may log freely.
    files: ["**/*.test.ts", "**/*.test.tsx", "e2e/**/*.ts"],
    rules: {
      "no-console": "off",
    },
  },
];

export default eslintConfig;
