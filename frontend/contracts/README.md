# Generated contract clients

Typed TypeScript clients for the contracts the frontend invokes, generated from
the WASM in [`contracts/`](../../contracts) with
`stellar contract bindings typescript` (SC-91 / #1420).

| Directory | Contract | Used for |
|---|---|---|
| `game_contract/` | `contracts/game_contract` | Puzzle-reward claims (`/puzzles`) |
| `yield_staking/` | `contracts/yield_staking` | Staking (`/stake`) |

## Do not edit by hand

Everything inside `game_contract/` and `yield_staking/` is committed generator
output — one typed method per contract function, each returning a simulated,
signable `AssembledTransaction`. Regenerate instead of editing:

```bash
# from frontend/
npm run bindings:generate
```

The script builds the contracts with `cargo` (wasm32-unknown-unknown) and runs
the stellar CLI, so both are required locally; `SKIP_WASM_BUILD=1` reuses an
already-built WASM. The bindings were last generated with stellar CLI 28.1.0.

`.github/workflows/contract-bindings.yml` regenerates the bindings on every PR
that touches `contracts/**` and fails if the committed output has drifted.

## Using the clients

Import the env-aware factories and typed flows from `@/contracts/client`:

- `createGameContractClient()` / `createYieldStakingClient()` — a client for a
  configured deployment.
- `claimPuzzleReward({ proof, signer })` — `claim_puzzle_reward`, signed by the
  connected Freighter wallet.
- `stakeInPool({ caller, pool, amount, signer })` — `yield_staking.stake`.
- `invokeBoundContract(...)` — generic dispatch used by
  `useAppContext().invokeBoundContract`.

Contracts are optional deployment config, read at call time from:

| Variable | Meaning |
|---|---|
| `NEXT_PUBLIC_GAME_CONTRACT_ID` | Deployed game contract (puzzle rewards) |
| `NEXT_PUBLIC_YIELD_STAKING_CONTRACT_ID` | Deployed yield-staking contract |
| `NEXT_PUBLIC_YIELD_STAKING_POOL_ADDRESS` | Whitelisted pool the stake goes into |

When they are unset the app keeps its pre-contract behaviour: `/stake` pays the
staking escrow account and puzzle rewards stay a local simulation.

## Adding another contract

1. Add the crate name to the `CONTRACTS` list in
   `scripts/generate-contract-bindings.sh`.
2. Run `npm run bindings:generate` and commit the new directory.
3. Re-export its `Client` from [`index.ts`](./index.ts) and add a factory in
   [`client.ts`](./client.ts).
