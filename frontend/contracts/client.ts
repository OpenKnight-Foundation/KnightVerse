/**
 * Env-aware factories and typed flows over the generated contract clients
 * (SC-91, #1420).
 *
 * Contract IDs are deployment config, so they are read at call time from
 * `NEXT_PUBLIC_*` variables. Every helper here degrades gracefully when a
 * contract is not deployed: callers check `getGameContractId()` /
 * `getYieldStakingConfig()` first and keep the pre-existing (non-contract)
 * behaviour when the binding is absent.
 */
import { Buffer } from "buffer";
import type {
  AssembledTransaction,
  ClientOptions,
  SentTransaction,
  SignTransaction,
} from "@stellar/stellar-sdk/contract";
import { NETWORK_PASSPHRASE, SOROBAN_RPC } from "@/lib/api";
import {
  GameContractClient,
  YieldStakingClient,
  TitleBadgeClient,
  AiNftClient,
  displayNameForMetadata,
  type ContractMethodInvoker,
  type PuzzleRewardProof,
} from "./index";

/** `NEXT_PUBLIC_GAME_CONTRACT_ID` — game contract (puzzle rewards, wagers). */
export function getGameContractId(): string | undefined {
  return process.env.NEXT_PUBLIC_GAME_CONTRACT_ID?.trim() || undefined;
}

export interface YieldStakingConfig {
  contractId: string;
  /** Address of the whitelisted liquidity pool the stake is routed into. */
  poolAddress: string;
}

/** `NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID` — yield-staking contract. */
export function getYieldStakingContractId(): string | undefined {
  return process.env.NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID?.trim() || undefined;
}

/**
 * `NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID` + `NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS`.
 * Both are required — staking without a pool is not a valid on-chain call.
 */
export function getYieldStakingConfig(): YieldStakingConfig | undefined {
  const contractId = getYieldStakingContractId();
  const poolAddress =
    process.env.NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS?.trim() || undefined;
  if (!contractId || !poolAddress) return undefined;
  return { contractId, poolAddress };
}

export function createGameContractClient(
  overrides: Partial<ClientOptions> = {},
): GameContractClient {
  const contractId = overrides.contractId ?? getGameContractId();
  if (!contractId) {
    throw new Error(
      "The game contract is not configured. Set NEXT_PUBLIC_GAME_CONTRACT_ID.",
    );
  }
  return new GameContractClient({
    rpcUrl: SOROBAN_RPC,
    networkPassphrase: NETWORK_PASSPHRASE,
    ...overrides,
    contractId,
  });
}

export function createYieldStakingClient(
  overrides: Partial<ClientOptions> = {},
): YieldStakingClient {
  const contractId = overrides.contractId ?? getYieldStakingConfig()?.contractId;
  if (!contractId) {
    throw new Error(
      "The yield staking contract is not configured. Set " +
        "NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID and NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS.",
    );
  }
  return new YieldStakingClient({
    rpcUrl: SOROBAN_RPC,
    networkPassphrase: NETWORK_PASSPHRASE,
    ...overrides,
    contractId,
  });
}

/**
 * The wallet surface the contract flows need. `sign` is the Freighter helper
 * exposed as `useAppContext().signTransaction`.
 */
export interface ContractSigner {
  /** Address that pays the fee and authorises the call. */
  address: string;
  /** Signs an envelope XDR and resolves to the signed envelope XDR. */
  sign: (txXdr: string) => Promise<string>;
}

/** Adapt the wallet's sign helper to the SDK's Freighter-shaped signer. */
function toSdkSigner(signer: ContractSigner): SignTransaction {
  return async (xdr, opts) => ({
    signedTxXdr: await signer.sign(xdr),
    signerAddress: opts?.address ?? signer.address,
  });
}

function hashOf(sent: SentTransaction<unknown>): string | undefined {
  return (
    sent.getTransactionResponse?.txHash ?? sent.sendTransactionResponse?.hash
  );
}

export interface ContractCallResult {
  /** Ledger transaction hash, once the call has been included on-chain. */
  hash?: string;
}

/**
 * A backend-signed win claim for `game_contract.claim_win` (FE-163).
 *
 * The signature is the result-signing service's ED25519 signature over
 * `SHA256(game_id_le8 || winner_address_bytes)`.
 */
export interface WinClaim {
  game_id: bigint;
  winner: string;
  signature: Buffer;
}

/** Soroban token amounts are 128-bit integers with 7 decimals. */
export function toTokenAmount(units: number, decimals = 7): bigint {
  if (!Number.isFinite(units)) {
    throw new TypeError(`Invalid token amount: ${units}`);
  }
  // `toFixed` keeps plain decimal notation (small values would otherwise come
  // back as exponent strings like "1e-7") and rounds to the token precision.
  const digits = units.toFixed(decimals).replace(".", "");
  if (!/^-?\d+$/.test(digits)) {
    throw new TypeError(`Amount out of range: ${units}`);
  }
  return BigInt(digits);
}

/**
 * Normalise a claim proof as received from the backend into the shape the
 * generated `game_contract` client expects (u64/i128 as bigint, signature as
 * bytes). Accepts the signature as base64/hex string or a byte array.
 */
export function normalizePuzzleRewardProof(value: unknown): PuzzleRewardProof {
  if (typeof value !== "object" || value === null) {
    throw new TypeError("Puzzle reward proof must be an object");
  }
  const raw = value as {
    recipient?: unknown;
    reward_amount?: unknown;
    nonce?: unknown;
    signature?: unknown;
  };
  if (typeof raw.recipient !== "string" || !raw.recipient) {
    throw new TypeError("Puzzle reward proof is missing `recipient`");
  }
  const rewardAmount = BigInt(raw.reward_amount as string | number | bigint);
  const nonce = BigInt(raw.nonce as string | number | bigint);
  const signature = toBytes(raw.signature);
  return {
    recipient: raw.recipient,
    reward_amount: rewardAmount,
    nonce,
    signature,
  };
}

/**
 * Best-effort variant of {@link normalizePuzzleRewardProof}: returns `null`
 * (and warns) instead of throwing, for payloads straight off the wire.
 */
export function parsePuzzleRewardProof(value: unknown): PuzzleRewardProof | null {
  if (!value) return null;
  try {
    return normalizePuzzleRewardProof(value);
  } catch (err) {
    console.warn("Ignoring malformed puzzle reward proof", err);
    return null;
  }
}

/**
 * Normalise a win-claim payload as received from the backend into the shape
 * the generated `game_contract` client expects. Accepts the game id as a
 * string/number/bigint and the signature as base64/hex string or byte array.
 */
export function normalizeWinClaim(value: unknown): WinClaim {
  if (typeof value !== "object" || value === null) {
    throw new TypeError("Win claim must be an object");
  }
  const raw = value as {
    game_id?: unknown;
    winner?: unknown;
    signature?: unknown;
  };
  if (raw.game_id === undefined || raw.game_id === null || raw.game_id === "") {
    throw new TypeError("Win claim is missing `game_id`");
  }
  if (typeof raw.winner !== "string" || !raw.winner) {
    throw new TypeError("Win claim is missing `winner`");
  }
  const signature = toBytes(raw.signature);
  if (signature.length !== 64) {
    throw new TypeError(
      `Win claim signature must be 64 bytes, received ${signature.length}`,
    );
  }
  return {
    game_id: BigInt(raw.game_id as string | number | bigint),
    winner: raw.winner,
    signature,
  };
}

/**
 * Best-effort variant of {@link normalizeWinClaim}: returns `null` (and warns)
 * instead of throwing, for payloads straight off the wire.
 */
export function parseWinClaim(value: unknown): WinClaim | null {
  if (!value) return null;
  try {
    return normalizeWinClaim(value);
  } catch (err) {
    console.warn("Ignoring malformed win claim", err);
    return null;
  }
}

function toBytes(value: unknown): Buffer {
  if (Buffer.isBuffer(value)) return value;
  if (value instanceof Uint8Array) return Buffer.from(value);
  if (Array.isArray(value)) return Buffer.from(value as number[]);
  if (typeof value === "string") {
    if (value.startsWith("0x")) return Buffer.from(value.slice(2), "hex");
    return Buffer.from(value, "base64");
  }
  throw new TypeError("Contract signature must be a byte array, hex or base64 string");
}

/**
 * Claim a puzzle reward on-chain via the generated game-contract client
 * (`claim_puzzle_reward`). The proof is the backend's ED25519 signature over
 * `(game_id | recipient) || (reward_amount || nonce)`; see
 * `contracts/game_contract`.
 */
export async function claimPuzzleReward(params: {
  proof: PuzzleRewardProof;
  signer: ContractSigner;
}): Promise<ContractCallResult> {
  const { proof, signer } = params;
  const client = createGameContractClient({
    publicKey: signer.address,
    signTransaction: toSdkSigner(signer),
  });
  const tx = await client.claim_puzzle_reward({
    recipient: proof.recipient,
    reward_amount: BigInt(proof.reward_amount),
    nonce: BigInt(proof.nonce),
    signature: Buffer.from(proof.signature),
  });
  const sent = await tx.signAndSend();
  return { hash: hashOf(sent) };
}

/**
 * Claim a staked game's winnings on-chain via the generated game-contract
 * client (`claim_win`, FE-163). The claim must carry the result-signing
 * service's ED25519 signature — the contract verifies it before paying out.
 */
export async function claimGameWin(params: {
  claim: WinClaim;
  signer: ContractSigner;
}): Promise<ContractCallResult> {
  const { claim, signer } = params;
  const client = createGameContractClient({
    publicKey: signer.address,
    signTransaction: toSdkSigner(signer),
  });
  const tx = await client.claim_win({
    game_id: BigInt(claim.game_id),
    winner: claim.winner,
    signature: Buffer.from(claim.signature),
  });
  const sent = await tx.signAndSend();
  return { hash: hashOf(sent) };
}

/**
 * Stake into a whitelisted liquidity pool via the generated
 * `yield_staking` client (`stake`). `amount` is in the staking contract's
 * token base units (7 decimals); use {@link toTokenAmount}.
 */
export async function stakeInPool(params: {
  caller: string;
  pool: string;
  amount: bigint;
  signer: ContractSigner;
}): Promise<ContractCallResult> {
  const { caller, pool, amount, signer } = params;
  const client = createYieldStakingClient({
    publicKey: caller,
    signTransaction: toSdkSigner(signer),
  });
  const tx = await client.stake({ caller, pool, amount });
  const sent = await tx.signAndSend();
  return { hash: hashOf(sent) };
}

export function getTitleBadgeContractId(): string | undefined {
  return process.env.NEXT_PUBLIC_CONTRACT_TITLE_BADGE?.trim() || undefined;
}

export function getAiNftContractId(): string | undefined {
  return process.env.NEXT_PUBLIC_CONTRACT_AI_NFT?.trim() || undefined;
}

export interface OnChainTitle {
  title: string;
  grantedAt: bigint;
}

const TITLE_BADGE_CACHE_TTL_MS = 5 * 60 * 1000;
const titleBadgeCache = new Map<
  string,
  { value: OnChainTitle | null; expiresAt: number }
>();

export function clearTitleBadgeCache(owner?: string): void {
  if (!owner) {
    titleBadgeCache.clear();
    return;
  }
  const suffix = `:${owner.toLowerCase()}`;
  for (const key of titleBadgeCache.keys()) {
    if (key.endsWith(suffix)) titleBadgeCache.delete(key);
  }
}

function isMissingTitleError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return /BadgeNotFound|AlreadyRevoked|revoked|not found|no badge/i.test(message);
}

export async function fetchOnChainTitle(params: {
  owner: string;
  invoke: ContractMethodInvoker;
  contractId?: string;
  forceRefresh?: boolean;
}): Promise<OnChainTitle | null> {
  const contractId = params.contractId ?? getTitleBadgeContractId();
  if (!contractId || !params.owner) return null;

  const key = `${contractId}:${params.owner.toLowerCase()}`;
  const cached = titleBadgeCache.get(key);
  if (!params.forceRefresh && cached && cached.expiresAt > Date.now()) {
    return cached.value;
  }

  try {
    const badge = await new TitleBadgeClient(params.invoke).get(params.owner);
    const value =
      badge && badge.active && badge.title.trim()
        ? { title: badge.title.trim(), grantedAt: badge.granted_at }
        : null;
    titleBadgeCache.set(key, {
      value,
      expiresAt: Date.now() + TITLE_BADGE_CACHE_TTL_MS,
    });
    return value;
  } catch (error) {
    if (!isMissingTitleError(error)) throw error;
    titleBadgeCache.set(key, {
      value: null,
      expiresAt: Date.now() + TITLE_BADGE_CACHE_TTL_MS,
    });
    return null;
  }
}

export interface AiNftCollectionItem {
  tokenId: bigint;
  nftId: bigint;
  owner: string;
  metadataHash: string;
  personalityTraits: string;
  createdAt: bigint;
  minter: string;
  displayName: string;
}

const AI_NFT_CACHE_TTL_MS = 30 * 1000;
const aiNftCache = new Map<
  string,
  { items: AiNftCollectionItem[]; expiresAt: number }
>();

export function clearAiNftCache(owner?: string): void {
  if (!owner) {
    aiNftCache.clear();
    return;
  }
  const suffix = `:${owner.toLowerCase()}`;
  for (const key of aiNftCache.keys()) {
    if (key.endsWith(suffix)) aiNftCache.delete(key);
  }
}

export async function fetchOwnedAiNfts(params: {
  owner: string;
  invoke: ContractMethodInvoker;
  contractId?: string;
  forceRefresh?: boolean;
}): Promise<AiNftCollectionItem[]> {
  const contractId = params.contractId ?? getAiNftContractId();
  if (!contractId || !params.owner) return [];

  const key = `${contractId}:${params.owner.toLowerCase()}`;
  const cached = aiNftCache.get(key);
  if (!params.forceRefresh && cached && cached.expiresAt > Date.now()) {
    return cached.items;
  }

  const client = new AiNftClient(params.invoke);
  const tokenIds = await client.tokensOf(params.owner);
  const settled = await Promise.all(
    tokenIds.map(async (tokenId) => {
      const metadata = await client.metadata(tokenId);
      if (!metadata) return null;
      const item: AiNftCollectionItem = {
        tokenId,
        nftId: metadata.nft_id,
        owner: metadata.owner,
        metadataHash: metadata.metadata_hash,
        personalityTraits: metadata.personality_traits,
        createdAt: metadata.created_at,
        minter: metadata.minter,
        displayName: displayNameForMetadata(metadata),
      };
      return item;
    }),
  );
  const items = settled.filter(
    (item): item is AiNftCollectionItem => item !== null,
  );
  aiNftCache.set(key, { items, expiresAt: Date.now() + AI_NFT_CACHE_TTL_MS });
  return items;
}

export async function transferOwnedAiNft(params: {
  tokenId: bigint;
  to: string;
  invoke: ContractMethodInvoker;
  owner?: string;
}): Promise<unknown> {
  const result = await new AiNftClient(params.invoke).transfer(
    params.tokenId,
    params.to,
  );
  if (params.owner) clearAiNftCache(params.owner);
  return result;
}

interface BoundContract {
  /** Crate/package name, used in error messages. */
  name: string;
  create: (overrides: Partial<ClientOptions>) => BoundContractClient;
}

/** The generated clients expose one method per contract function. */
type BoundContractClient = Record<string, unknown>;

/** Look up the generated bindings for a deployed contract id, if any. */
export function resolveBoundContract(contractId: string): BoundContract | undefined {
  const gameContractId = getGameContractId();
  if (gameContractId && contractId === gameContractId) {
    return {
      name: "game_contract",
      create: (overrides) =>
        createGameContractClient(overrides) as unknown as BoundContractClient,
    };
  }
  const staking = getYieldStakingConfig();
  if (staking && contractId === staking.contractId) {
    return {
      name: "yield_staking",
      create: (overrides) =>
        createYieldStakingClient(overrides) as unknown as BoundContractClient,
    };
  }
  return undefined;
}

type GeneratedMethod = (
  args: Record<string, unknown>,
  options?: Partial<ClientOptions>,
) => Promise<AssembledTransaction<unknown>>;

/**
 * Invoke a contract method through its generated bindings, keyed by
 * `NEXT_PUBLIC_*_CONTRACT_ID`. Read calls are simulated and resolved to their
 * result; write calls are signed by the wallet and submitted.
 *
 * Arguments are passed as a record keyed by the contract's parameter names —
 * the same names the generated client's methods take.
 */
export async function invokeBoundContract(params: {
  contractId: string;
  method: string;
  args?: Record<string, unknown>;
  signer: ContractSigner;
}): Promise<unknown> {
  const { contractId, method, args, signer } = params;
  const bound = resolveBoundContract(contractId);
  if (!bound) {
    throw new Error(
      `No TypeScript bindings are registered for contract ${contractId}. ` +
        "Regenerate them with `npm run bindings:generate` and point the " +
        "matching NEXT_PUBLIC_*_CONTRACT_ID variable at the deployment.",
    );
  }
  const client = bound.create({
    contractId,
    publicKey: signer.address,
    signTransaction: toSdkSigner(signer),
  }) as BoundContractClient;
  const invoke = client[method];
  if (typeof invoke !== "function") {
    throw new Error(`Contract ${bound.name} has no method '${method}'.`);
  }
  const tx = await (invoke as GeneratedMethod).call(client, args ?? {}, {
    publicKey: signer.address,
    signTransaction: toSdkSigner(signer),
  });
  if (tx.isReadCall) return tx.result;
  const sent = await tx.signAndSend();
  return sent.result;
}
