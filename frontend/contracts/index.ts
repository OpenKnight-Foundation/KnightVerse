/**
 * Typed contract clients for the frontend (SC-91, #1420).
 *
 * `game_contract/` and `yield_staking/` are complete NPM packages generated
 * from the WASM in `contracts/` by `npm run bindings:generate`
 * (`stellar contract bindings typescript`). They are committed output — never
 * edit them by hand; re-run the script instead. CI regenerates them and fails
 * when the committed bindings drift from the contracts.
 *
 * Both generated packages export a `Client` class and re-export the whole
 * `@stellar/stellar-sdk`, so they are re-exported here under unambiguous
 * names. Use `@/contracts/client` for the env-aware factories and the typed
 * staking / puzzle-claim flows the app actually calls.
 */
export { Client as GameContractClient } from "./game_contract/src/index";
export type { Proof as PuzzleRewardProof } from "./game_contract/src/index";
export { Client as YieldStakingClient } from "./yield_staking/src/index";
export type { PoolState } from "./yield_staking/src/index";
