/**
 * Debug logger — dev-only console output (#FE-130).
 *
 * Socket hooks, the wallet/sound contexts and the PGN exporters used to call
 * `console.log` directly, which shipped verbose traces (including full move
 * payloads) to production browsers. Application code now routes those traces
 * through this helper, which compiles down to a no-op in production builds.
 *
 * `console.warn` / `console.error` are deliberately *not* routed here: they
 * report real problems and stay in production. The `no-console` ESLint rule in
 * `eslint.config.mjs` enforces exactly that split.
 */

/**
 * `process.env.NODE_ENV` is statically inlined by Next.js, so this folds away
 * to `false` in a production bundle and the `console.log` call is dropped by
 * the minifier's dead-code elimination.
 */
const isEnabled = process.env.NODE_ENV !== "production";

/** Emit a development-only trace. No-op in production. */
export function debugLog(...args: unknown[]): void {
  if (!isEnabled) return;
  // eslint-disable-next-line no-console -- the single sanctioned console.log in the app
  console.log(...args);
}