import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




export type DataKey = {tag: "Admin", values: void} | {tag: "Treasury", values: void} | {tag: "Token", values: void} | {tag: "Organizers", values: void} | {tag: "PoolCounter", values: void} | {tag: "Whitelist", values: void} | {tag: "Escrow", values: void} | {tag: "WalletYield", values: void} | {tag: "Entered", values: void};


/**
 * On-chain state of one staked tournament prize position.
 */
export interface PoolState {
  /**
 * Whether the organizer explicitly accepted principal risk.
 */
allow_principal_loss: boolean;
  /**
 * Yield harvested from this position so far.
 */
harvested_yield: i128;
  /**
 * Liquidity pool contract the principal is staked in.
 */
pool: string;
  /**
 * Whether the tournament has been settled and the position closed.
 */
settled: boolean;
  /**
 * Total underlying tokens staked (principal only; excludes harvested
 * yield, which is credited to the treasury escrow when harvested).
 */
total_staked: i128;
}

/**
 * Errors surfaced through the generated `try_*` client methods.
 */
export const ContractError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"Unauthorized"},
  4: {message:"InvalidAmount"},
  5: {message:"PoolNotWhitelisted"},
  6: {message:"PoolAlreadyWhitelisted"},
  7: {message:"TournamentAlreadySettled"},
  8: {message:"ReentrantCall"},
  /**
   * Settlement would return less than the deposited principal and the
   * organizer did not accept principal risk (zero-loss mode).
   */
  9: {message:"PrincipalLoss"},
  /**
   * Percentages must be within 1..=100.
   */
  10: {message:"InvalidPercentage"},
  /**
   * Percentages must sum to exactly 100.
   */
  11: {message:"PercentageSumMismatch"},
  /**
   * At least one winner is required for settlement.
   */
  12: {message:"EmptyWinners"},
  /**
   * The requested position does not exist.
   */
  13: {message:"PoolNotFound"},
  /**
   * Checked arithmetic overflowed.
   */
  14: {message:"ArithmeticOverflow"},
  /**
   * The pool's underlying token does not match the contract's fixed token.
   */
  15: {message:"TokenMismatch"}
}

export interface Client {
  /**
   * Construct and simulate a admin transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  admin: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a stake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Lock `amount` of the caller's tournament prize pool into the
   * whitelisted liquidity `pool`.
   * 
   * The tokens are pulled from the caller (the tournament organizer) into
   * this contract, which then forwards them to the pool via the standard
   * pool interface (`deposit(from, amount)`). Each (organizer, pool) pair
   * holds exactly one position and further stakes top it up. Returns the
   * position id.
   */
  stake: ({caller, pool, amount}: {caller: string, pool: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a token transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The underlying token fixed by the first deposit (if any).
   */
  token: (options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>

  /**
   * Construct and simulate a unstake transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Unstake `amount` of principal from the caller's own position back to
   * the caller (e.g. the tournament was cancelled) without settling.
   */
  unstake: ({caller, pool_id, amount}: {caller: string, pool_id: u64, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Position state for (organizer, pool_id), if it exists.
   */
  position: ({organizer, pool_id}: {organizer: string, pool_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<PoolState>>>

  /**
   * Construct and simulate a treasury transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  treasury: (options?: MethodOptions) => Promise<AssembledTransaction<Result<string>>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Initialize the contract with an admin (whitelists, staking, settling)
   * and a treasury (recipient of harvested yield).
   */
  initialize: ({admin, treasury}: {admin: string, treasury: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a position_id transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Look up the position id an organizer holds with a given pool.
   */
  position_id: ({organizer, pool}: {organizer: string, pool: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<u64>>>

  /**
   * Construct and simulate a claim_escrow transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claim escrowed tokens. Each address may claim only its own credited
   * balance; winners claim their payout share and the treasury claims
   * harvested yield.
   */
  claim_escrow: ({caller, amount}: {caller: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a accrued_yield transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Yield accrued on top of the tracked principal, according to the pool's
   * own accounting (`balance_of`).
   */
  accrued_yield: ({pool_id}: {pool_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a harvest_yield transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Harvest accrued yield for a position: withdraw it from the pool into
   * the contract and credit it to the treasury's escrow so it can be added
   * to the final payout distribution. The principal stays staked and keeps
   * earning. Returns the amount harvested by this call.
   */
  harvest_yield: ({caller, pool_id}: {caller: string, pool_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a escrow_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Escrowed payout balance credited to `who` by settlement/harvesting.
   */
  escrow_balance: ({who}: {who: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a whitelist_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Whitelist a liquidity pool contract. Only whitelisted addresses can
   * receive principal from `stake` — a compromised admin cannot silently
   * point prize funds at an arbitrary "pool" that was never reviewed.
   */
  whitelist_pool: ({caller, pool}: {caller: string, pool: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a unwhitelist_pool transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Remove a liquidity pool from the whitelist. Existing positions are
   * unaffected — they continue to harvest and settle normally.
   */
  unwhitelist_pool: ({caller, pool}: {caller: string, pool: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a settle_tournament transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Settle a concluded tournament: verify principal safety, withdraw the
   * full position (principal + accrued yield) from the pool, and credit
   * the whole balance to `winners` by `percentages` in the escrow ledger —
   * mirroring the `payout_tournament` convention of `game_contract`
   * (percentages 1..=100 summing to exactly 100; division dust goes to the
   * first winner so the distribution is always exact). Previously
   * harvested-but-unclaimed yield is included in the distribution.
   */
  settle_tournament: ({caller, pool_id, winners, percentages}: {caller: string, pool_id: u64, winners: Array<string>, percentages: Array<i128>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a is_pool_whitelisted transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  is_pool_whitelisted: ({pool}: {pool: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

  /**
   * Construct and simulate a organizer_position_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  organizer_position_count: ({organizer}: {organizer: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a set_allow_principal_loss transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Explicitly accept (or revoke) principal risk for a position. Only the
   * admin may do this; it is required before a settlement that would
   * return less than the deposited principal.
   */
  set_allow_principal_loss: ({caller, pool_id, allow}: {caller: string, pool_id: u64, allow: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAAAAAAAFYWRtaW4AAAAAAAAAAAAAAQAAA+kAAAATAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAX5Mb2NrIGBhbW91bnRgIG9mIHRoZSBjYWxsZXIncyB0b3VybmFtZW50IHByaXplIHBvb2wgaW50byB0aGUKd2hpdGVsaXN0ZWQgbGlxdWlkaXR5IGBwb29sYC4KClRoZSB0b2tlbnMgYXJlIHB1bGxlZCBmcm9tIHRoZSBjYWxsZXIgKHRoZSB0b3VybmFtZW50IG9yZ2FuaXplcikgaW50bwp0aGlzIGNvbnRyYWN0LCB3aGljaCB0aGVuIGZvcndhcmRzIHRoZW0gdG8gdGhlIHBvb2wgdmlhIHRoZSBzdGFuZGFyZApwb29sIGludGVyZmFjZSAoYGRlcG9zaXQoZnJvbSwgYW1vdW50KWApLiBFYWNoIChvcmdhbml6ZXIsIHBvb2wpIHBhaXIKaG9sZHMgZXhhY3RseSBvbmUgcG9zaXRpb24gYW5kIGZ1cnRoZXIgc3Rha2VzIHRvcCBpdCB1cC4gUmV0dXJucyB0aGUKcG9zaXRpb24gaWQuAAAAAAAFc3Rha2UAAAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABHBvb2wAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAAAYAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAADlUaGUgdW5kZXJseWluZyB0b2tlbiBmaXhlZCBieSB0aGUgZmlyc3QgZGVwb3NpdCAoaWYgYW55KS4AAAAAAAAFdG9rZW4AAAAAAAAAAAAAAQAAA+gAAAAT",
        "AAAAAAAAAIVVbnN0YWtlIGBhbW91bnRgIG9mIHByaW5jaXBhbCBmcm9tIHRoZSBjYWxsZXIncyBvd24gcG9zaXRpb24gYmFjayB0bwp0aGUgY2FsbGVyIChlLmcuIHRoZSB0b3VybmFtZW50IHdhcyBjYW5jZWxsZWQpIHdpdGhvdXQgc2V0dGxpbmcuAAAAAAAAB3Vuc3Rha2UAAAAAAwAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAdwb29sX2lkAAAAAAYAAAAAAAAABmFtb3VudAAAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAADZQb3NpdGlvbiBzdGF0ZSBmb3IgKG9yZ2FuaXplciwgcG9vbF9pZCksIGlmIGl0IGV4aXN0cy4AAAAAAAhwb3NpdGlvbgAAAAIAAAAAAAAACW9yZ2FuaXplcgAAAAAAABMAAAAAAAAAB3Bvb2xfaWQAAAAABgAAAAEAAAPoAAAH0AAAAAlQb29sU3RhdGUAAAA=",
        "AAAAAAAAAAAAAAAIdHJlYXN1cnkAAAAAAAAAAQAAA+kAAAATAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAACQAAAAAAAABBQWRkcmVzcyBhbGxvd2VkIHRvIHdoaXRlbGlzdCBwb29scywgc3Rha2UgYW5kIHNldHRsZSB0b3VybmFtZW50cy4AAAAAAAAFQWRtaW4AAAAAAAAAAAAAMEFkZHJlc3MgYWxsb3dlZCB0byBjbGFpbSBoYXJ2ZXN0ZWQteWllbGQgZXNjcm93LgAAAAhUcmVhc3VyeQAAAAAAAAAzVW5kZXJseWluZyB0b2tlbiBjb250cmFjdCAoZml4ZWQgb24gZmlyc3QgZGVwb3NpdCkuAAAAAAVUb2tlbgAAAAAAAAAAAAA5YE1hcDxBZGRyZXNzIChvcmdhbml6ZXIpLCBNYXA8dTY0IChwb29sX2lkKSwgUG9vbFN0YXRlPj5gAAAAAAAACk9yZ2FuaXplcnMAAAAAAAAAAAAaTW9ub3RvbmljIHBvb2wgaWQgY291bnRlci4AAAAAAAtQb29sQ291bnRlcgAAAAAAAAAANVNldCBvZiB3aGl0ZWxpc3RlZCBsaXF1aWRpdHkgcG9vbCBjb250cmFjdCBhZGRyZXNzZXMuAAAAAAAACVdoaXRlbGlzdAAAAAAAAAAAAAAvYE1hcDxBZGRyZXNzIChyZWNpcGllbnQpLCBpMTI4PmAgZXNjcm93IGxlZGdlci4AAAAABkVzY3JvdwAAAAAAAAAAAIZVbmRlcmx5aW5nIHRva2VucyBoZWxkIGluIHRoZSBjb250cmFjdCB3YWxsZXQgdGhhdCBhcmUgYXR0cmlidXRhYmxlIHRvCmhhcnZlc3RlZC1idXQtbm90LXlldC1kaXN0cmlidXRlZCB5aWVsZCAoYWNyb3NzIGFsbCBwb3NpdGlvbnMpLgAAAAAAC1dhbGxldFlpZWxkAAAAAAAAAAAWUmVlbnRyYW5jeSBndWFyZCBmbGFnLgAAAAAAB0VudGVyZWQA",
        "AAAAAAAAAHRJbml0aWFsaXplIHRoZSBjb250cmFjdCB3aXRoIGFuIGFkbWluICh3aGl0ZWxpc3RzLCBzdGFraW5nLCBzZXR0bGluZykKYW5kIGEgdHJlYXN1cnkgKHJlY2lwaWVudCBvZiBoYXJ2ZXN0ZWQgeWllbGQpLgAAAAppbml0aWFsaXplAAAAAAACAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAACHRyZWFzdXJ5AAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAD1Mb29rIHVwIHRoZSBwb3NpdGlvbiBpZCBhbiBvcmdhbml6ZXIgaG9sZHMgd2l0aCBhIGdpdmVuIHBvb2wuAAAAAAAAC3Bvc2l0aW9uX2lkAAAAAAIAAAAAAAAACW9yZ2FuaXplcgAAAAAAABMAAAAAAAAABHBvb2wAAAATAAAAAQAAA+gAAAAG",
        "AAAAAQAAADdPbi1jaGFpbiBzdGF0ZSBvZiBvbmUgc3Rha2VkIHRvdXJuYW1lbnQgcHJpemUgcG9zaXRpb24uAAAAAAAAAAAJUG9vbFN0YXRlAAAAAAAABQAAADlXaGV0aGVyIHRoZSBvcmdhbml6ZXIgZXhwbGljaXRseSBhY2NlcHRlZCBwcmluY2lwYWwgcmlzay4AAAAAAAAUYWxsb3dfcHJpbmNpcGFsX2xvc3MAAAABAAAAKllpZWxkIGhhcnZlc3RlZCBmcm9tIHRoaXMgcG9zaXRpb24gc28gZmFyLgAAAAAAD2hhcnZlc3RlZF95aWVsZAAAAAALAAAAM0xpcXVpZGl0eSBwb29sIGNvbnRyYWN0IHRoZSBwcmluY2lwYWwgaXMgc3Rha2VkIGluLgAAAAAEcG9vbAAAABMAAABAV2hldGhlciB0aGUgdG91cm5hbWVudCBoYXMgYmVlbiBzZXR0bGVkIGFuZCB0aGUgcG9zaXRpb24gY2xvc2VkLgAAAAdzZXR0bGVkAAAAAAEAAACDVG90YWwgdW5kZXJseWluZyB0b2tlbnMgc3Rha2VkIChwcmluY2lwYWwgb25seTsgZXhjbHVkZXMgaGFydmVzdGVkCnlpZWxkLCB3aGljaCBpcyBjcmVkaXRlZCB0byB0aGUgdHJlYXN1cnkgZXNjcm93IHdoZW4gaGFydmVzdGVkKS4AAAAADHRvdGFsX3N0YWtlZAAAAAs=",
        "AAAAAAAAAJZDbGFpbSBlc2Nyb3dlZCB0b2tlbnMuIEVhY2ggYWRkcmVzcyBtYXkgY2xhaW0gb25seSBpdHMgb3duIGNyZWRpdGVkCmJhbGFuY2U7IHdpbm5lcnMgY2xhaW0gdGhlaXIgcGF5b3V0IHNoYXJlIGFuZCB0aGUgdHJlYXN1cnkgY2xhaW1zCmhhcnZlc3RlZCB5aWVsZC4AAAAAAAxjbGFpbV9lc2Nyb3cAAAACAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmFtb3VudAAAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAGVZaWVsZCBhY2NydWVkIG9uIHRvcCBvZiB0aGUgdHJhY2tlZCBwcmluY2lwYWwsIGFjY29yZGluZyB0byB0aGUgcG9vbCdzCm93biBhY2NvdW50aW5nIChgYmFsYW5jZV9vZmApLgAAAAAAAA1hY2NydWVkX3lpZWxkAAAAAAAAAQAAAAAAAAAHcG9vbF9pZAAAAAAGAAAAAQAAA+kAAAALAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAQZIYXJ2ZXN0IGFjY3J1ZWQgeWllbGQgZm9yIGEgcG9zaXRpb246IHdpdGhkcmF3IGl0IGZyb20gdGhlIHBvb2wgaW50bwp0aGUgY29udHJhY3QgYW5kIGNyZWRpdCBpdCB0byB0aGUgdHJlYXN1cnkncyBlc2Nyb3cgc28gaXQgY2FuIGJlIGFkZGVkCnRvIHRoZSBmaW5hbCBwYXlvdXQgZGlzdHJpYnV0aW9uLiBUaGUgcHJpbmNpcGFsIHN0YXlzIHN0YWtlZCBhbmQga2VlcHMKZWFybmluZy4gUmV0dXJucyB0aGUgYW1vdW50IGhhcnZlc3RlZCBieSB0aGlzIGNhbGwuAAAAAAANaGFydmVzdF95aWVsZAAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAHcG9vbF9pZAAAAAAGAAAAAQAAA+kAAAALAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAAAAAAAENFc2Nyb3dlZCBwYXlvdXQgYmFsYW5jZSBjcmVkaXRlZCB0byBgd2hvYCBieSBzZXR0bGVtZW50L2hhcnZlc3RpbmcuAAAAAA5lc2Nyb3dfYmFsYW5jZQAAAAAAAQAAAAAAAAADd2hvAAAAABMAAAABAAAACw==",
        "AAAAAAAAAMxXaGl0ZWxpc3QgYSBsaXF1aWRpdHkgcG9vbCBjb250cmFjdC4gT25seSB3aGl0ZWxpc3RlZCBhZGRyZXNzZXMgY2FuCnJlY2VpdmUgcHJpbmNpcGFsIGZyb20gYHN0YWtlYCDigJQgYSBjb21wcm9taXNlZCBhZG1pbiBjYW5ub3Qgc2lsZW50bHkKcG9pbnQgcHJpemUgZnVuZHMgYXQgYW4gYXJiaXRyYXJ5ICJwb29sIiB0aGF0IHdhcyBuZXZlciByZXZpZXdlZC4AAAAOd2hpdGVsaXN0X3Bvb2wAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAEcG9vbAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1Db250cmFjdEVycm9yAAAA",
        "AAAABAAAAD1FcnJvcnMgc3VyZmFjZWQgdGhyb3VnaCB0aGUgZ2VuZXJhdGVkIGB0cnlfKmAgY2xpZW50IG1ldGhvZHMuAAAAAAAAAAAAAA1Db250cmFjdEVycm9yAAAAAAAADwAAAAAAAAASQWxyZWFkeUluaXRpYWxpemVkAAAAAAABAAAAAAAAAA5Ob3RJbml0aWFsaXplZAAAAAAAAgAAAAAAAAAMVW5hdXRob3JpemVkAAAAAwAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAQAAAAAAAAAElBvb2xOb3RXaGl0ZWxpc3RlZAAAAAAABQAAAAAAAAAWUG9vbEFscmVhZHlXaGl0ZWxpc3RlZAAAAAAABgAAAAAAAAAYVG91cm5hbWVudEFscmVhZHlTZXR0bGVkAAAABwAAAAAAAAANUmVlbnRyYW50Q2FsbAAAAAAAAAgAAAB7U2V0dGxlbWVudCB3b3VsZCByZXR1cm4gbGVzcyB0aGFuIHRoZSBkZXBvc2l0ZWQgcHJpbmNpcGFsIGFuZCB0aGUKb3JnYW5pemVyIGRpZCBub3QgYWNjZXB0IHByaW5jaXBhbCByaXNrICh6ZXJvLWxvc3MgbW9kZSkuAAAAAA1QcmluY2lwYWxMb3NzAAAAAAAACQAAACNQZXJjZW50YWdlcyBtdXN0IGJlIHdpdGhpbiAxLi49MTAwLgAAAAARSW52YWxpZFBlcmNlbnRhZ2UAAAAAAAAKAAAAJFBlcmNlbnRhZ2VzIG11c3Qgc3VtIHRvIGV4YWN0bHkgMTAwLgAAABVQZXJjZW50YWdlU3VtTWlzbWF0Y2gAAAAAAAALAAAAL0F0IGxlYXN0IG9uZSB3aW5uZXIgaXMgcmVxdWlyZWQgZm9yIHNldHRsZW1lbnQuAAAAAAxFbXB0eVdpbm5lcnMAAAAMAAAAJlRoZSByZXF1ZXN0ZWQgcG9zaXRpb24gZG9lcyBub3QgZXhpc3QuAAAAAAAMUG9vbE5vdEZvdW5kAAAADQAAAB5DaGVja2VkIGFyaXRobWV0aWMgb3ZlcmZsb3dlZC4AAAAAABJBcml0aG1ldGljT3ZlcmZsb3cAAAAAAA4AAABGVGhlIHBvb2wncyB1bmRlcmx5aW5nIHRva2VuIGRvZXMgbm90IG1hdGNoIHRoZSBjb250cmFjdCdzIGZpeGVkIHRva2VuLgAAAAAADVRva2VuTWlzbWF0Y2gAAAAAAAAP",
        "AAAAAAAAAH9SZW1vdmUgYSBsaXF1aWRpdHkgcG9vbCBmcm9tIHRoZSB3aGl0ZWxpc3QuIEV4aXN0aW5nIHBvc2l0aW9ucyBhcmUKdW5hZmZlY3RlZCDigJQgdGhleSBjb250aW51ZSB0byBoYXJ2ZXN0IGFuZCBzZXR0bGUgbm9ybWFsbHkuAAAAABB1bndoaXRlbGlzdF9wb29sAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAARwb29sAAAAEwAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADUNvbnRyYWN0RXJyb3IAAAA=",
        "AAAAAAAAAdVTZXR0bGUgYSBjb25jbHVkZWQgdG91cm5hbWVudDogdmVyaWZ5IHByaW5jaXBhbCBzYWZldHksIHdpdGhkcmF3IHRoZQpmdWxsIHBvc2l0aW9uIChwcmluY2lwYWwgKyBhY2NydWVkIHlpZWxkKSBmcm9tIHRoZSBwb29sLCBhbmQgY3JlZGl0CnRoZSB3aG9sZSBiYWxhbmNlIHRvIGB3aW5uZXJzYCBieSBgcGVyY2VudGFnZXNgIGluIHRoZSBlc2Nyb3cgbGVkZ2VyIOKAlAptaXJyb3JpbmcgdGhlIGBwYXlvdXRfdG91cm5hbWVudGAgY29udmVudGlvbiBvZiBgZ2FtZV9jb250cmFjdGAKKHBlcmNlbnRhZ2VzIDEuLj0xMDAgc3VtbWluZyB0byBleGFjdGx5IDEwMDsgZGl2aXNpb24gZHVzdCBnb2VzIHRvIHRoZQpmaXJzdCB3aW5uZXIgc28gdGhlIGRpc3RyaWJ1dGlvbiBpcyBhbHdheXMgZXhhY3QpLiBQcmV2aW91c2x5CmhhcnZlc3RlZC1idXQtdW5jbGFpbWVkIHlpZWxkIGlzIGluY2x1ZGVkIGluIHRoZSBkaXN0cmlidXRpb24uAAAAAAAAEXNldHRsZV90b3VybmFtZW50AAAAAAAABAAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAdwb29sX2lkAAAAAAYAAAAAAAAAB3dpbm5lcnMAAAAD6gAAABMAAAAAAAAAC3BlcmNlbnRhZ2VzAAAAA+oAAAALAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==",
        "AAAAAAAAAAAAAAATaXNfcG9vbF93aGl0ZWxpc3RlZAAAAAABAAAAAAAAAARwb29sAAAAEwAAAAEAAAAB",
        "AAAAAAAAAAAAAAAYb3JnYW5pemVyX3Bvc2l0aW9uX2NvdW50AAAAAQAAAAAAAAAJb3JnYW5pemVyAAAAAAAAEwAAAAEAAAAE",
        "AAAAAAAAALBFeHBsaWNpdGx5IGFjY2VwdCAob3IgcmV2b2tlKSBwcmluY2lwYWwgcmlzayBmb3IgYSBwb3NpdGlvbi4gT25seSB0aGUKYWRtaW4gbWF5IGRvIHRoaXM7IGl0IGlzIHJlcXVpcmVkIGJlZm9yZSBhIHNldHRsZW1lbnQgdGhhdCB3b3VsZApyZXR1cm4gbGVzcyB0aGFuIHRoZSBkZXBvc2l0ZWQgcHJpbmNpcGFsLgAAABhzZXRfYWxsb3dfcHJpbmNpcGFsX2xvc3MAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAAB3Bvb2xfaWQAAAAABgAAAAAAAAAFYWxsb3cAAAAAAAABAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANQ29udHJhY3RFcnJvcgAAAA==" ]),
      options
    )
  }
  public readonly fromJSON = {
    admin: this.txFromJSON<Result<string>>,
        stake: this.txFromJSON<Result<u64>>,
        token: this.txFromJSON<Option<string>>,
        unstake: this.txFromJSON<Result<void>>,
        position: this.txFromJSON<Option<PoolState>>,
        treasury: this.txFromJSON<Result<string>>,
        initialize: this.txFromJSON<Result<void>>,
        position_id: this.txFromJSON<Option<u64>>,
        claim_escrow: this.txFromJSON<Result<void>>,
        accrued_yield: this.txFromJSON<Result<i128>>,
        harvest_yield: this.txFromJSON<Result<i128>>,
        escrow_balance: this.txFromJSON<i128>,
        whitelist_pool: this.txFromJSON<Result<void>>,
        unwhitelist_pool: this.txFromJSON<Result<void>>,
        settle_tournament: this.txFromJSON<Result<void>>,
        is_pool_whitelisted: this.txFromJSON<boolean>,
        organizer_position_count: this.txFromJSON<u32>,
        set_allow_principal_loss: this.txFromJSON<Result<void>>
  }
}