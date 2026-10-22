/**
 * Soroban Typed Contract Client Types
 *
 * Types for contract simulation, signing, submission, and polling,
 * as well as strongly typed parameters and return values for
 * Staking, Puzzle Rewards, AI NFTs, and Escrow contracts.
 */

export interface InvokeContractOptions {
  /**
   * If true, only simulates the invocation on-chain and returns the
   * simulation return value without requesting signatures or submitting.
   */
  simulateOnly?: boolean;

  /**
   * Custom base fee in stroops. Defaults to Horizon's base fee.
   */
  fee?: string | number;

  /**
   * Transaction timeout in seconds. Defaults to 60s.
   */
  timeoutInSeconds?: number;

  /**
   * Polling interval in milliseconds while waiting for transaction confirmation.
   * Defaults to 1000ms (1 second).
   */
  pollIntervalMs?: number;

  /**
   * Maximum number of polling attempts before giving up.
   * Defaults to 30 attempts.
   */
  maxPollAttempts?: number;
}

export interface InvocationResult<T = unknown> {
  status: "SUCCESS";
  hash: string;
  returnValue: T;
  raw?: unknown;
}

export type ContractInvoker = (
  contractId: string,
  functionName: string,
  args?: unknown[],
  options?: InvokeContractOptions,
) => Promise<InvocationResult | unknown>;

// ── Contract Domain Models ──────────────────────────────────────────────────

/** SC-46 Yield Staking Position */
export interface StakingPosition {
  pool: string;
  total_staked: bigint;
  harvested_yield: bigint;
  settled: boolean;
  allow_principal_loss: boolean;
}

/** AI-41 / AI-NFT Metadata */
export interface AINFTMetadata {
  name: string;
  symbol: string;
  uri: string;
  owner: string;
  minter: string;
}

/** SC-45 Arbiter Match Escrow */
export interface MatchEscrow {
  match_id: bigint;
  player1: string;
  player2: string;
  arbiter: string;
  amount: bigint;
  token: string;
  state: number;
}

/** Puzzle Reward Proof */
export interface PuzzleRewardProof {
  puzzle_id: number | bigint;
  player: string;
  solution_hash: string;
  timestamp: number | bigint;
}
