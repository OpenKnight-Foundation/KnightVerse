/**
 * In-memory-only holder for the current access token (#FE-166).
 *
 * The token is never written to localStorage/sessionStorage, so a successful
 * XSS payload can no longer read it out of storage; it can only be reached by
 * code running in the same tab's memory. Non-React modules (plain fetch
 * helpers) read it from here instead of `window.localStorage`.
 */
let currentAccessToken: string | null = null;

export function getSessionAccessToken(): string | null {
  return currentAccessToken;
}

export function setSessionAccessToken(token: string | null): void {
  currentAccessToken = token;
}
