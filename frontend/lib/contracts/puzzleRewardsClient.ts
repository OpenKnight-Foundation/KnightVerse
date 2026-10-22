/**
 * Puzzle Rewards Typed Contract Client
 *
 * Interacts with KnightVerse game_contract puzzle rewards and validation.
 */

import { TypedContractClient } from "./baseClient";
import { InvokeContractOptions, InvocationResult, PuzzleRewardProof } from "./types";

export class PuzzleRewardsContractClient extends TypedContractClient {
  /**
   * Claims reward for completing an individual puzzle.
   */
  async claimPuzzleReward(
    player: string,
    puzzleId: number | bigint,
    rewardAmount: bigint | number | string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke(
      "claim_puzzle_reward",
      [player, BigInt(puzzleId), BigInt(rewardAmount)],
      options,
    );
  }

  /**
   * Batch claims multiple puzzle rewards in a single transaction.
   */
  async claimPuzzleRewardsBatch(
    proofs: PuzzleRewardProof[],
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("claim_puzzle_rewards_batch", [proofs], options);
  }

  /**
   * Submits a puzzle solution on-chain.
   */
  async submitSolution(
    caller: string,
    puzzleId: number | bigint,
    solution: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("submit_solution", [caller, BigInt(puzzleId), solution], options);
  }

  /**
   * Reads accumulated puzzle reward balance for an address (read-only query).
   */
  async getRewardBalance(player: string): Promise<bigint> {
    return this.query<bigint>("reward_balance", [player]);
  }

  /**
   * Reads on-chain puzzle information (read-only query).
   */
  async getPuzzle(puzzleId: number | bigint): Promise<unknown> {
    return this.query<unknown>("get_puzzle", [BigInt(puzzleId)]);
  }

  /**
   * Reads total count of on-chain puzzles (read-only query).
   */
  async getPuzzleCount(): Promise<number> {
    return this.query<number>("puzzle_count", []);
  }
}
