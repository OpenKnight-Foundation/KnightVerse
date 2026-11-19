import type { ContractMethodInvoker } from "../../title_badge/src/index";

export interface AiNftMetadata {
  owner: string;
  nft_id: bigint;
  metadata_hash: string;
  personality_traits: string;
  created_at: bigint;
  minter: string;
}

export interface OwnedAiNft extends AiNftMetadata {
  token_id: bigint;
  display_name: string;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null
    ? (value as Record<string, unknown>)
    : null;
}

function toBigInt(value: unknown): bigint {
  if (typeof value === "bigint") return value;
  if (typeof value === "number") return BigInt(Math.trunc(value));
  if (typeof value === "string" && value.trim()) return BigInt(value);
  if (typeof value === "object" && value !== null && "_value" in value) {
    return toBigInt((value as { _value: unknown })._value);
  }
  throw new TypeError("Invalid bigint value");
}

function safeBigInt(value: unknown): bigint {
  try {
    return toBigInt(unwrap(value));
  } catch {
    return 0n;
  }
}

function unwrap(value: unknown): unknown {
  let current = value;
  for (let depth = 0; depth < 6; depth += 1) {
    const record = asRecord(current);
    if (record && "_value" in record) {
      current = record._value;
      continue;
    }
    if (Array.isArray(current) && current.length === 2 && typeof current[0] === "boolean") {
      if (!current[0]) return null;
      current = current[1];
      continue;
    }
    break;
  }
  return current;
}

function addressOf(value: unknown): string {
  const unwrapped = unwrap(value);
  if (typeof unwrapped === "string") return unwrapped;
  if (unwrapped && typeof unwrapped === "object") {
    const record = asRecord(unwrapped);
    if (record && typeof record.address === "string") return record.address;
  }
  return unwrapped == null ? "" : String(unwrapped);
}

function bytesToHex(value: unknown): string {
  const unwrapped = unwrap(value);
  if (typeof unwrapped === "string") {
    return unwrapped.startsWith("0x") ? unwrapped : `0x${unwrapped}`;
  }
  if (unwrapped instanceof Uint8Array) {
    return `0x${Array.from(unwrapped, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
  }
  if (Array.isArray(unwrapped)) {
    return `0x${unwrapped
      .map((byte) => Number(byte).toString(16).padStart(2, "0"))
      .join("")}`;
  }
  return "";
}

export function parseTokenIds(value: unknown): bigint[] {
  const unwrapped = unwrap(value);
  const values = Array.isArray(unwrapped)
    ? unwrapped
    : unwrapped instanceof Map
      ? Array.from(unwrapped.keys())
      : [];
  return values
    .map((entry) => safeBigInt(unwrap(entry)))
    .filter((id) => id >= 0n);
}

export function displayNameForMetadata(metadata: AiNftMetadata): string {
  const raw = metadata.personality_traits.trim();
  if (!raw) return `AI NFT #${metadata.nft_id}`;

  try {
    const parsed: unknown = JSON.parse(raw);
    const record = asRecord(parsed);
    if (record) {
      const name = record.name ?? record.title ?? record.personality;
      if (typeof name === "string" && name.trim()) return name.trim();
    }
  } catch {
    return raw.replace(/[_-]+/g, " ").replace(/\s+/g, " ").trim();
  }

  return raw.replace(/\s+/g, " ").trim();
}

export function normalizeAiNftMetadata(
  value: unknown,
  fallbackTokenId?: bigint,
): AiNftMetadata | null {
  if (!value) return null;

  const unwrapped = unwrap(value);
  const raw = Array.isArray(unwrapped)
    ? {
        owner: unwrapped[0],
        nft_id: unwrapped[1],
        metadata_hash: unwrapped[2],
        personality_traits: unwrapped[3],
        created_at: unwrapped[4],
        minter: unwrapped[5],
      }
    : asRecord(unwrapped);
  if (!raw) return null;

  const owner = addressOf(raw.owner);
  if (!owner) return null;

  return {
    owner,
    nft_id: safeBigInt(unwrap(raw.nft_id ?? fallbackTokenId)),
    metadata_hash: bytesToHex(raw.metadata_hash),
    personality_traits:
      typeof raw.personality_traits === "string"
        ? raw.personality_traits
        : "",
    created_at: safeBigInt(unwrap(raw.created_at)),
    minter: addressOf(raw.minter),
  };
}

export class AiNftClient {
  constructor(private readonly invoke: ContractMethodInvoker) {}

  async tokensOf(owner: string): Promise<bigint[]> {
    try {
      const result = await this.invoke("tokens_of", [owner], {
        simulateOnly: true,
      });
      return parseTokenIds(result);
    } catch {
      return this.fallbackTokensOf(owner);
    }
  }

  async metadata(nftId: bigint): Promise<AiNftMetadata | null> {
    const result = await this.invoke("metadata", [nftId], {
      simulateOnly: true,
    });
    return normalizeAiNftMetadata(result, nftId);
  }

  async transfer(nftId: bigint, to: string): Promise<unknown> {
    return this.invoke("transfer", [nftId, to]);
  }

  private async fallbackTokensOf(owner: string): Promise<bigint[]> {
    const supply = safeBigInt(
      unwrap(await this.invoke("total_supply", [], { simulateOnly: true })),
    );
    if (supply > 10000n) {
      throw new Error("Too many AI NFTs to enumerate without tokens_of");
    }

    const ownerLower = owner.toLowerCase();
    const ids: bigint[] = [];
    for (let id = 0n; id < supply; id++) {
      try {
        const result = await this.invoke("owner_of", [id], {
          simulateOnly: true,
        });
        if (addressOf(result).toLowerCase() === ownerLower) ids.push(id);
      } catch {
        continue;
      }
    }
    return ids;
  }
}
