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

export async function fetchNativeBalance(address: string, network: StellarNetwork = "mainnet"): Promise<number> {
  const response = await fetch(`${HORIZON_URLS[network]}/accounts/${encodeURIComponent(address)}`, { cache: "no-store" });
  if (!response.ok) throw new Error("Unable to load wallet balance");
  const data = (await response.json()) as { balances?: Array<{ asset_type: string; balance: string }> };
  return Number(data.balances?.find((balance) => balance.asset_type === "native")?.balance ?? 0);
}