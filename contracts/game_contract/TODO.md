# SC-02: Missing Game State Persistence in create_tournament_escrow (#772)

## Steps
- [x] Step 1: Read and understand the relevant code (lib.rs lines 1916-1971)
- [x] Step 2: Create plan and get approval
- [x] Step 3: Add `Escrowed` variant to `GameState` enum
- [x] Step 4: Fix `create_tournament_escrow` - add state checks + persist game state
- [x] Step 5: Fix `release_tournament_escrow` - update game state to `Settled` after release
- [x] Step 6: Fix `payout` function - add explicit `Escrowed` state rejection
- [x] Step 7: All changes verified in source code

