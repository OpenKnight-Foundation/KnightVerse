"use client";

import { useCallback, useRef } from "react";
import { useTransactionContext } from "@/context/transactionContext";
import type { TxType } from "@/context/transactionContext";
import { useToast } from "@/components/ui/toast";

// ---------------------------------------------------------------------------
// Horizon polling helper (FE-16 fix)
// ---------------------------------------------------------------------------

const HORIZON_URL =
  process.env.NEXT_PUBLIC_HORIZON_URL ?? "https://horizon-testnet.stellar.org";

interface HorizonTxRecord {
  successful: boolean;
  id: string;
  hash: string;
}

/**
 * Poll the Stellar Horizon API until a transaction with the given `hash`
 * reaches `successful` status or the `signal` is aborted.
 *
 * Implements a simple exponential-backoff retry (1 s → 2 s → 4 s … capped
 * at 16 s) so we don't hammer the API.  Throws if polling is aborted or if
 * the transaction is ultimately marked unsuccessful by the network.
 */
async function pollHorizonForConfirmation(
  hash: string,
  signal: AbortSignal,
  maxAttempts = 20,
): Promise<HorizonTxRecord> {
  let attempt = 0;
  const BASE_DELAY_MS = 1_000;
  const MAX_DELAY_MS = 16_000;

  while (attempt < maxAttempts) {
    if (signal.aborted) {
      throw new DOMException("Polling aborted", "AbortError");
    }

    try {
      const url = `${HORIZON_URL}/transactions/${encodeURIComponent(hash)}`;
      const res = await fetch(url, { signal });

      if (res.ok) {
        const tx: HorizonTxRecord = await res.json();
        if (!tx.successful) {
          throw new Error(`Transaction ${hash} was rejected by the network.`);
        }
        return tx;
      }

      // 404 means the transaction is not yet visible on the ledger — keep polling.
      if (res.status !== 404) {
        // Unexpected error from Horizon; surface it immediately.
        const body = await res.text().catch(() => res.statusText);
        throw new Error(`Horizon returned ${res.status}: ${body}`);
      }
    } catch (err) {
      // Re-throw AbortError (user cancelled or component unmounted)
      if (err instanceof DOMException && err.name === "AbortError") throw err;
      // Re-throw definitive failures (not a 404 / network hiccup)
      if (err instanceof Error && !err.message.startsWith("Horizon returned 404")) {
        // Only re-throw if it's not a transient network error
        const isNetworkError =
          err.message.includes("Failed to fetch") || err.message.includes("NetworkError");
        if (!isNetworkError) throw err;
      }
    }

    // Back-off before the next attempt
    const delay = Math.min(BASE_DELAY_MS * Math.pow(2, attempt), MAX_DELAY_MS);
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(resolve, delay);
      signal.addEventListener("abort", () => {
        clearTimeout(timer);
        reject(new DOMException("Polling aborted", "AbortError"));
      }, { once: true });
    });

    attempt++;
  }

  throw new Error(
    `Transaction ${hash} was not confirmed after ${maxAttempts} polling attempts.`,
  );
}

// ---------------------------------------------------------------------------
// Hook options and return type
// ---------------------------------------------------------------------------

interface UseTrackedTransactionOptions {
  /** Transaction type */
  type: TxType;
  /** Human-readable label */
  label: string;
  /** Amount for display */
  amount?: string;
  /** Destination for payments */
  destination?: string;
  /** Auto-dismiss after confirmation (ms). Default: 8000 */
  autoDismissMs?: number;
}

/**
 * Hook that wraps an async operation (e.g. sendXLM) with full
 * transaction lifecycle tracking and toast notifications.
 *
 * The wrapped `fn` must resolve to an object that exposes a `hash` property
 * (the Stellar transaction hash returned by Horizon's submit endpoint).
 * Once the hash is obtained the hook polls Horizon to confirm inclusion
 * before transitioning to the "confirmed" phase (FE-16 fix).
 *
 * CPU-efficient: only creates state updates when the phase changes,
 * and auto-dismisses confirmed transactions after a configurable delay.
 */
export function useTrackedTransaction(opts: UseTrackedTransactionOptions) {
  const { startTransaction, updatePhase, dismissTransaction } =
    useTransactionContext();
  const { addToast } = useToast();
  const autoDismissMs = opts.autoDismissMs ?? 8000;
  const dismissTimerRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(
    new Map(),
  );
  // AbortController for the current polling operation; aborted on unmount
  // via the cleanup return of the calling component or explicitly.
  const pollingAbortControllerRef = useRef<AbortController | null>(null);

  const execute = useCallback(
    async <T extends { hash?: string }>(
      fn: (txId: string) => Promise<T>,
    ): Promise<T | undefined> => {
      // Abort any previous polling that might still be running
      if (pollingAbortControllerRef.current) {
        pollingAbortControllerRef.current.abort();
      }
      const abortController = new AbortController();
      pollingAbortControllerRef.current = abortController;

      // Phase 1: Preparing
      const txId = startTransaction({
        type: opts.type,
        phase: "preparing",
        label: opts.label,
        amount: opts.amount,
        destination: opts.destination,
      });

      try {
        // Phase 2: Signing (the fn should call Freighter sign)
        updatePhase(txId, "signing");

        const result = await fn(txId);

        // Phase 3: Submitting
        updatePhase(txId, "submitting");

        // Phase 4: Confirming — poll Horizon for actual inclusion.
        updatePhase(txId, "confirming");

        const txHash = result?.hash;

        if (txHash) {
          // Wait for the network to confirm the transaction.
          await pollHorizonForConfirmation(txHash, abortController.signal);
        } else {
          // No hash available (e.g. dry-run or mock env): log a warning and
          // proceed so existing callers that don't yet return a hash aren't broken.
          console.warn(
            "[useTrackedTransaction] fn() resolved without a `hash` field — " +
            "skipping Horizon polling.  Ensure sendXLM / contract calls return { hash }.",
          );
        }

        // Phase 5: Confirmed
        updatePhase(txId, "confirmed");

        addToast({
          severity: "success",
          title: "Transaction Confirmed",
          detail: `${opts.label} completed successfully.`,
        });

        // Auto-dismiss after delay
        const timer = setTimeout(() => {
          dismissTransaction(txId);
          dismissTimerRef.current.delete(txId);
        }, autoDismissMs);
        dismissTimerRef.current.set(txId, timer);

        return result;
      } catch (err) {
        // Ignore AbortError from intentional cancellation (component unmount)
        if (err instanceof DOMException && err.name === "AbortError") {
          return undefined;
        }

        const message =
          err instanceof Error ? err.message : "Transaction failed.";

        updatePhase(txId, "failed", { error: message });

        addToast({
          severity: "error",
          title: "Transaction Failed",
          detail: message,
        });

        return undefined;
      }
    },
    [
      opts.type,
      opts.label,
      opts.amount,
      opts.destination,
      autoDismissMs,
      startTransaction,
      updatePhase,
      dismissTransaction,
      addToast,
    ],
  );

  return { execute };
}
