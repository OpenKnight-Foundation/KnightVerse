export type SupportedToken = "XLM" | "USDC" | "EURC";

const tokenIds: Record<SupportedToken, string> = { XLM: "stellar", USDC: "usd-coin", EURC: "euro-coin" };

export async function fetchTokenPrices(): Promise<Record<SupportedToken, number>> {
  const ids = Object.values(tokenIds).join(",");
  const response = await fetch(`https://api.coingecko.com/api/v3/simple/price?ids=${ids}&vs_currencies=usd`, { next: { revalidate: 30 } });
  if (!response.ok) throw new Error("Unable to load token prices");
  const data = (await response.json()) as Record<string, { usd?: number }>;
  return { XLM: data[tokenIds.XLM]?.usd ?? 0, USDC: data[tokenIds.USDC]?.usd ?? 1, EURC: data[tokenIds.EURC]?.usd ?? 1.08 };
}

export type StellarNetwork = "mainnet" | "testnet";

export const HORIZON_URLS: Record<StellarNetwork, string> = {
  mainnet: "https://horizon.stellar.org",
  testnet: "https://horizon-testnet.stellar.org",
};

/**
 * Circle's issuing accounts for USDC and EURC. Asset codes are not unique on
 * Stellar (anyone can issue a "USDC"), so a balance only counts when both the
 * code and the issuer match.
 */
export const TOKEN_ISSUERS: Record<StellarNetwork, Record<Exclude<SupportedToken, "XLM">, string>> = {
  mainnet: {
    USDC: "GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN",
    EURC: "GDHU6WRG4IEQXM5NZ4BMPKOXHW76MZM4Y2IEMFDVXBSDP6SJY4ITNPP2",
  },
  testnet: {
    USDC: "GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5",
    EURC: "GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO",
  },
};

type HorizonBalance = { asset_type: string; asset_code?: string; asset_issuer?: string; balance: string };

/** Loads the wallet's XLM, USDC and EURC balances in one Horizon request. A missing trustline counts as 0. */
export async function fetchWalletBalances(address: string, network: StellarNetwork = "mainnet"): Promise<Record<SupportedToken, number>> {
  const response = await fetch(`${HORIZON_URLS[network]}/accounts/${encodeURIComponent(address)}`, { cache: "no-store" });
  if (!response.ok) throw new Error("Unable to load wallet balance");
  const data = (await response.json()) as { balances?: HorizonBalance[] };
  const balances = data.balances ?? [];
  const issued = (code: Exclude<SupportedToken, "XLM">) =>
    Number(balances.find((b) => b.asset_type !== "native" && b.asset_code === code && b.asset_issuer === TOKEN_ISSUERS[network][code])?.balance ?? 0);
  return {
    XLM: Number(balances.find((b) => b.asset_type === "native")?.balance ?? 0),
    USDC: issued("USDC"),
    EURC: issued("EURC"),
  };
}

export async function fetchNativeBalance(address: string, network: StellarNetwork = "mainnet"): Promise<number> {
  return (await fetchWalletBalances(address, network)).XLM;
}
