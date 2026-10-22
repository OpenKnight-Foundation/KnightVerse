/**
 * Yield Staking Typed Contract Client
 *
 * Interacts with SC-46 Automated Liquidity Pool Yield Staking contract.
 */

import { TypedContractClient } from "./baseClient";
import { InvokeContractOptions, InvocationResult, StakingPosition } from "./types";

export class YieldStakingContractClient extends TypedContractClient {
  /**
   * Stakes principal into a whitelisted liquidity pool.
   */
  async stake(
    organizer: string,
    pool: string,
    amount: bigint | number | string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("stake", [organizer, pool, BigInt(amount)], options);
  }

  /**
   * Unstakes principal from a liquidity pool.
   */
  async unstake(
    organizer: string,
    pool: string,
    amount: bigint | number | string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("unstake", [organizer, pool, BigInt(amount)], options);
  }

  /**
   * Harvests accumulated yield from a staked position.
   */
  async harvestYield(
    caller: string,
    poolId: number | bigint,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult<bigint>> {
    return this.invoke<bigint>("harvest_yield", [caller, BigInt(poolId)], options);
  }

  /**
   * Claims available escrow balance for the caller.
   */
  async claimEscrow(
    caller: string,
    amount: bigint | number | string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("claim_escrow", [caller, BigInt(amount)], options);
  }

  /**
   * Reads escrow balance for an address (read-only query).
   */
  async getEscrowBalance(who: string): Promise<bigint> {
    return this.query<bigint>("escrow_balance", [who]);
  }

  /**
   * Reads a tournament pool position by ID (read-only query).
   */
  async getPosition(
    organizer: string,
    poolId: number | bigint,
  ): Promise<StakingPosition | null> {
    return this.query<StakingPosition | null>("position", [organizer, BigInt(poolId)]);
  }

  /**
   * Checks whether a liquidity pool is whitelisted (read-only query).
   */
  async isPoolWhitelisted(pool: string): Promise<boolean> {
    return this.query<boolean>("is_pool_whitelisted", [pool]);
  }

  /**
   * Reads contract admin (read-only query).
   */
  async getAdmin(): Promise<string> {
    return this.query<string>("admin", []);
  }

  /**
   * Reads treasury address (read-only query).
   */
  async getTreasury(): Promise<string> {
    return this.query<string>("treasury", []);
  }
}
