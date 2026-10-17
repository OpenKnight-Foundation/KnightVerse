import { useCallback, useState } from "react";

import { endpoints } from "@/lib/api";
import { useAuth } from "@/context/authContext";
import { useAppContext } from "@/context/walletContext";

type FreighterSignResult =
  | string
  | { signedMessage?: string | Uint8Array | { data: number[] } | null; error?: unknown };

type FreighterLike = {
  signMessage?: (message: string, opts?: { address?: string }) => Promise<FreighterSignResult>;
};

function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  bytes.forEach((b) => (binary += String.fromCharCode(b)));
  return btoa(binary);
}

/**
 * Normalises the different shapes Freighter versions return from
 * `signMessage` into a base64 signature string.
 */
export function normalizeSignature(result: FreighterSignResult): string {
  if (typeof result === "string") return result;
  if (result?.error) throw new Error(String((result.error as { message?: string })?.message ?? result.error));
  const signed = result?.signedMessage;
  if (!signed) throw new Error("Freighter did not return a signature.");
  if (typeof signed === "string") return signed;
  if (signed instanceof Uint8Array) return bytesToBase64(signed);
  if (Array.isArray((signed as { data?: number[] }).data)) {
    return bytesToBase64(Uint8Array.from((signed as { data: number[] }).data));
  }
  throw new Error("Unrecognised Freighter signature format.");
}

function getFreighter(): FreighterLike | undefined {
  if (typeof window === "undefined") return undefined;
  const w = window as unknown as { freighterApi?: FreighterLike; freighter?: FreighterLike };
  return w.freighterApi ?? w.freighter;
}

async function readError(res: Response, fallback: string): Promise<string> {
  try {
    const body = await res.json();
    return body?.message ?? fallback;
  } catch {
    return fallback;
  }
}

/**
 * Links the connected Freighter wallet to the signed-in account (#FE-97):
 * request a challenge, have Freighter sign it, and let the backend verify the
 * signature against the address before storing it on the player.
 */
export function useWalletLink() {
  const { accessToken, user, setLinkedWallet } = useAuth();
  const { address: connectedAddress } = useAppContext();
  const [isLinking, setIsLinking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const linkedWallet = user?.wallet_address ?? null;

  const linkWallet = useCallback(async () => {
    setError(null);
    if (!accessToken) return setError("Sign in before linking a wallet.");
    if (!connectedAddress) return setError("Connect your Freighter wallet first.");
    const freighter = getFreighter();
    if (!freighter?.signMessage) {
      return setError("This Freighter version can't sign messages. Please update Freighter.");
    }

    setIsLinking(true);
    try {
      const headers = { "Content-Type": "application/json", Authorization: `Bearer ${accessToken}` };
      const challengeRes = await fetch(endpoints.players.walletChallenge(), {
        method: "POST",
        headers,
        body: JSON.stringify({ wallet_address: connectedAddress }),
      });
      if (!challengeRes.ok) throw new Error(await readError(challengeRes, "Could not start wallet linking."));
      const { message } = (await challengeRes.json()) as { message: string };

      const signature = normalizeSignature(await freighter.signMessage(message, { address: connectedAddress }));

      const verifyRes = await fetch(endpoints.players.walletVerify(), {
        method: "POST",
        headers,
        body: JSON.stringify({ wallet_address: connectedAddress, signature }),
      });
      if (!verifyRes.ok) throw new Error(await readError(verifyRes, "Wallet verification failed."));
      const { wallet_address } = (await verifyRes.json()) as { wallet_address: string | null };
      setLinkedWallet(wallet_address);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Wallet linking failed.");
    } finally {
      setIsLinking(false);
    }
  }, [accessToken, connectedAddress, setLinkedWallet]);

  const unlinkWallet = useCallback(async () => {
    setError(null);
    if (!accessToken) return;
    setIsLinking(true);
    try {
      const res = await fetch(endpoints.players.wallet(), {
        method: "DELETE",
        headers: { Authorization: `Bearer ${accessToken}` },
      });
      if (!res.ok) throw new Error(await readError(res, "Could not unlink wallet."));
      setLinkedWallet(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not unlink wallet.");
    } finally {
      setIsLinking(false);
    }
  }, [accessToken, setLinkedWallet]);

  return {
    linkedWallet,
    connectedAddress: connectedAddress ?? null,
    /** True when a wallet is connected but differs from the linked one. */
    isMismatched: Boolean(linkedWallet && connectedAddress && linkedWallet !== connectedAddress),
    isLinking,
    error,
    linkWallet,
    unlinkWallet,
  };
}
