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
      // FE-169 / FE-130: Enforce no-console as an error, allowing console.error for error boundaries.
      "no-console": ["error", { allow: ["error"] }],
    },
  },
  {
    // Allow console.error in error boundaries specifically if needed, or keep standard error boundary file overrides
    files: ["**/ErrorBoundary.tsx", "**/error.tsx"],
    rules:
      {
        "no-console": ["error", { allow: ["error"] }],
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
