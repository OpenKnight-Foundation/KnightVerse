/**
 * Arbiter Escrow Typed Contract Client
 *
 * Interacts with SC-45 Multi-Sig Arbiter Escrow contract.
 */

import { TypedContractClient } from "./baseClient";
import { InvokeContractOptions, InvocationResult, MatchEscrow } from "./types";

export class ArbiterEscrowContractClient extends TypedContractClient {
  /**
   * Creates an escrow for a match between two players with a designated arbiter.
   */
  async createEscrow(
    matchId: number | bigint,
    player1: string,
    player2: string,
    arbiter: string,
    amount: bigint | number | string,
    token: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke(
      "create_escrow",
      [BigInt(matchId), player1, player2, arbiter, BigInt(amount), token],
      options,
    );
  }

  /**
   * Deposits a player's share into the escrow.
   */
  async fundEscrow(
    matchId: number | bigint,
    player: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("fund_escrow", [BigInt(matchId), player], options);
  }

  /**
   * Settles a cleanly finished match by releasing funds to the winner.
   */
  async settleMatch(
    matchId: number | bigint,
    admin: string,
    winner: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("settle_match", [BigInt(matchId), admin, winner], options);
  }

  /**
   * Refunds match funds back to players.
   */
  async settleRefund(
    matchId: number | bigint,
    admin: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("settle_refund", [BigInt(matchId), admin], options);
  }

  /**
   * Initiates a dispute for an active match escrow.
   */
  async disputeMatch(
    matchId: number | bigint,
    caller: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("dispute_match", [BigInt(matchId), caller], options);
  }

  /**
   * Submits an arbiter vote on a disputed match.
   */
  async vote(
    matchId: number | bigint,
    arbiter: string,
    choice: number,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("vote", [BigInt(matchId), arbiter, choice], options);
  }

  /**
   * Resolves a dispute once voting quorum has been reached.
   */
  async resolveDispute(
    matchId: number | bigint,
    caller: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("resolve_dispute", [BigInt(matchId), caller], options);
  }

  /**
   * Reads escrow details for a match (read-only query).
   */
  async getEscrow(matchId: number | bigint): Promise<MatchEscrow | null> {
    return this.query<MatchEscrow | null>("get_escrow", [BigInt(matchId)]);
  }

  /**
   * Reads escrow contract admin (read-only query).
   */
  async getAdmin(): Promise<string> {
    return this.query<string>("get_admin", []);
  }
}
