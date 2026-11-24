/**
 * Freighter network checks (FE-164).
 *
 * A wallet on a different network than the app silently signs envelopes the
 * target network rejects, so every signature is preceded by a comparison of
 * the wallet's passphrase with `NETWORK_PASSPHRASE`.
 */

export const MAINNET_PASSPHRASE = "Public Global Stellar Network ; September 2015";
export const TESTNET_PASSPHRASE = "Test SDF Network ; September 2015";
export const FUTURENET_PASSPHRASE = "Test SDF Future Network ; October 2022";
export const SANDBOX_PASSPHRASE = "Local Sandbox Stellar Network ; September 2022";

/** Human-readable name for a passphrase, for use in messages and UI. */
const NAME_BY_PASSPHRASE: Record<string, string> = {
  [MAINNET_PASSPHRASE]: "Mainnet",
  [TESTNET_PASSPHRASE]: "Testnet",
  [FUTURENET_PASSPHRASE]: "Futurenet",
  [SANDBOX_PASSPHRASE]: "Sandbox",
};

/**
 * Freighter's `getNetwork()` returns a network *name*, while
 * `getNetworkDetails()` returns the passphrase. Older builds only expose the
 * name, so map the known names onto their passphrases.
 */
const PASSPHRASE_BY_NETWORK_NAME: Record<string, string> = {
  PUBLIC: MAINNET_PASSPHRASE,
  MAINNET: MAINNET_PASSPHRASE,
  TESTNET: TESTNET_PASSPHRASE,
  FUTURENET: FUTURENET_PASSPHRASE,
  SANDBOX: SANDBOX_PASSPHRASE,
  STANDALONE: SANDBOX_PASSPHRASE,
};

/** "Mainnet" / "Testnet" / … — falls back to the raw passphrase. */
export function describeNetwork(passphrase?: string | null): string {
  if (!passphrase) return "an unknown network";
  return NAME_BY_PASSPHRASE[passphrase] ?? `"${passphrase}"`;
}

/** Maps a Freighter network name (`PUBLIC`, `TESTNET`, …) to its passphrase. */
export function passphraseFromNetworkName(name?: string | null): string | undefined {
  if (typeof name !== "string") return undefined;
  return PASSPHRASE_BY_NETWORK_NAME[name.trim().toUpperCase()];
}

/**
 * True only when both passphrases are known and differ. An unreadable wallet
 * network is "unknown", not "mismatched" — blocking on it would lock out
 * wallets that cannot report their network at all.
 */
export function isNetworkMismatch(
  walletPassphrase?: string | null,
  expectedPassphrase?: string | null,
): boolean {
  if (!walletPassphrase || !expectedPassphrase) return false;
  return walletPassphrase !== expectedPassphrase;
}

/** The message shown when signing is blocked because of a network mismatch. */
export function networkMismatchMessage(
  walletPassphrase: string,
  expectedPassphrase: string,
): string {
  return (
    `Your Freighter wallet is on ${describeNetwork(walletPassphrase)}, but ` +
    `KnightVerse needs ${describeNetwork(expectedPassphrase)}. ` +
    `Switch networks in Freighter, then try again.`
  );
}

/**
 * Reads the passphrase out of whatever shape Freighter returned: a details
 * object (`{ networkPassphrase }`), a bare passphrase, or a network name.
 */
export function passphraseFromWalletResponse(response: unknown): string | undefined {
  if (typeof response === "string") {
    return passphraseFromNetworkName(response) ?? response;
  }
  if (!response || typeof response !== "object") return undefined;
  const details = response as {
    networkPassphrase?: unknown;
    network?: unknown;
  };
  if (typeof details.networkPassphrase === "string" && details.networkPassphrase) {
    return details.networkPassphrase;
  }
  return passphraseFromNetworkName(
    typeof details.network === "string" ? details.network : undefined,
  );
}
