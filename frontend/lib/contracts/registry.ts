/**
 * Typed Contract Registry & Factory
 *
 * Centralizes instantiation and lookup of typed contract clients
 * for Staking, Puzzle Rewards, AI NFTs, and Escrow.
 */

import { TypedContractClient } from "./baseClient";
import { ContractInvoker } from "./types";
import { YieldStakingContractClient } from "./stakingClient";
import { PuzzleRewardsContractClient } from "./puzzleRewardsClient";
import { AiNftContractClient } from "./nftClient";
import { ArbiterEscrowContractClient } from "./escrowClient";

export type KnownContractType = "staking" | "puzzle" | "nft" | "escrow";

/** In-memory registry for initialized typed contract clients */
const clientRegistry = new Map<string, TypedContractClient>();

/**
 * Registers an existing typed contract client instance for a given contract ID.
 */
export function registerTypedClient(contractId: string, client: TypedContractClient): void {
  clientRegistry.set(contractId, client);
}

/**
 * Looks up a previously registered typed contract client by its contract ID.
 */
export function findRegisteredClient<T extends TypedContractClient = TypedContractClient>(
  contractId: string,
): T | undefined {
  return clientRegistry.get(contractId) as T | undefined;
}

/**
 * Factory that instantiates a typed contract client based on contract type.
 */
export function createTypedContractClient(
  type: KnownContractType | string,
  contractId: string,
  invoker: ContractInvoker,
): TypedContractClient {
  let client: TypedContractClient;

  switch (type.toLowerCase()) {
    case "staking":
    case "yield_staking":
      client = new YieldStakingContractClient(contractId, invoker);
      break;

    case "puzzle":
    case "puzzles":
    case "puzzle_rewards":
    case "game":
      client = new PuzzleRewardsContractClient(contractId, invoker);
      break;

    case "nft":
    case "ai_nft":
      client = new AiNftContractClient(contractId, invoker);
      break;

    case "escrow":
    case "arbiter_escrow":
      client = new ArbiterEscrowContractClient(contractId, invoker);
      break;

    default:
      // Generic typed wrapper fallback
      client = new (class extends TypedContractClient {})(contractId, invoker);
      break;
  }

  registerTypedClient(contractId, client);
  return client;
}
