/**
 * Tests for fetchWithRetry (#FE-75)
 *
 * Vitest is configured with globals:true so describe/it/expect/vi are
 * available without explicit imports.  We still import them explicitly here
 * for editor type-checking clarity.
 *
 * @vitest-environment jsdom
 */
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { fetchWithRetry } from '@/lib/api';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Build a minimal Response-like object accepted by the mocked global.fetch. */
function makeResponse(status: number, ok = status >= 200 && status < 300): Response {
  return {
    status,
    ok,
    headers: new Headers(),
    redirected: false,
    statusText: String(status),
    type: 'basic',
    url: '',
    body: null,
    bodyUsed: false,
    clone: function () { return this; },
    arrayBuffer: async () => new ArrayBuffer(0),
    blob: async () => new Blob(),
    formData: async () => new FormData(),
    json: async () => ({}),
    text: async () => '',
  } as unknown as Response;
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

beforeEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// GET retries — succeeds after N failures
// ---------------------------------------------------------------------------

describe('fetchWithRetry — GET (idempotent)', () => {
  it('returns the successful response when a GET fails twice then succeeds', async () => {
    const okResponse = makeResponse(200);
    const failResponse = makeResponse(503);

    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      // First two calls return 503, third returns 200.
      .mockResolvedValueOnce(failResponse)
      .mockResolvedValueOnce(failResponse)
      .mockResolvedValueOnce(okResponse);

    const result = await fetchWithRetry(
      'http://localhost/test',
      { method: 'GET' },
      { maxRetries: 3, baseDelayMs: 0 },
    );

    expect(result.status).toBe(200);
    expect(result.ok).toBe(true);
    // fetch was called 3 times total (2 failures + 1 success)
    expect(fetchMock).toHaveBeenCalledTimes(3);
  });

  it('throws when all GET retries are exhausted (network error)', async () => {
    const networkError = new TypeError('Failed to fetch');

    vi.spyOn(globalThis, 'fetch').mockRejectedValue(networkError);

    await expect(
      fetchWithRetry(
        'http://localhost/test',
        { method: 'GET' },
        { maxRetries: 2, baseDelayMs: 0 },
      ),
    ).rejects.toThrow('Failed to fetch');
  });

  it('throws when all retries are exhausted with 503 responses', async () => {
    // Because the function returns the last response (not throws) on HTTP
    // error, check that the returned response has a failing status.
    const failResponse = makeResponse(503);

    vi.spyOn(globalThis, 'fetch').mockResolvedValue(failResponse);

    const result = await fetchWithRetry(
      'http://localhost/test',
      { method: 'GET' },
      { maxRetries: 2, baseDelayMs: 0 },
    );

    expect(result.ok).toBe(false);
    expect(result.status).toBe(503);
  });
});

// ---------------------------------------------------------------------------
// POST — NOT retried by default
// ---------------------------------------------------------------------------

describe('fetchWithRetry — POST (non-idempotent)', () => {
  it('does NOT retry a POST request on 503 by default', async () => {
    const failResponse = makeResponse(503);
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(failResponse);

    const result = await fetchWithRetry(
      'http://localhost/test',
      { method: 'POST', body: JSON.stringify({ x: 1 }) },
      { maxRetries: 3, baseDelayMs: 0 },
    );

    // Should return the failed response immediately, without retrying.
    expect(result.status).toBe(503);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('does NOT retry a POST request on a network error by default', async () => {
    const networkError = new TypeError('Failed to fetch');
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockRejectedValue(networkError);

    await expect(
      fetchWithRetry(
        'http://localhost/test',
        { method: 'POST' },
        { maxRetries: 3, baseDelayMs: 0 },
      ),
    ).rejects.toThrow('Failed to fetch');

    // fetch called exactly once — no retry for POST.
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('retries a POST when retryNonIdempotent is true', async () => {
    const failResponse = makeResponse(503);
    const okResponse = makeResponse(201);

    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(failResponse)
      .mockResolvedValueOnce(okResponse);

    const result = await fetchWithRetry(
      'http://localhost/test',
      { method: 'POST' },
      { maxRetries: 2, baseDelayMs: 0, retryNonIdempotent: true },
    );

    expect(result.status).toBe(201);
    expect(fetchMock).toHaveBeenCalledTimes(2);
  });
});

// ---------------------------------------------------------------------------
// Retry exhaustion — throws
// ---------------------------------------------------------------------------

describe('fetchWithRetry — retry exhaustion', () => {
  it('throws the last error after all retries are exhausted (GET network error)', async () => {
    const networkError = new Error('network down');
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockRejectedValue(networkError);

    await expect(
      fetchWithRetry(
        'http://localhost/test',
        undefined,
        { maxRetries: 3, baseDelayMs: 0 },
      ),
    ).rejects.toThrow('network down');

    // 1 initial attempt + 3 retries = 4 total calls.
    expect(fetchMock).toHaveBeenCalledTimes(4);
  });

  it('respects maxRetries: 0 (no retries at all)', async () => {
    const failResponse = makeResponse(500);
    const fetchMock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(failResponse);

    const result = await fetchWithRetry(
      'http://localhost/test',
      { method: 'GET' },
      { maxRetries: 0, baseDelayMs: 0 },
    );

    expect(result.ok).toBe(false);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
