# Issue #1551: Unverified Game Settlement Through `submit_move`

## Summary

The game contract's `submit_move` entry point accepts a client-provided game
status and currently uses it to settle results. Because the status is not
validated by the contract or authenticated by a backend signature, move
submission can cause an unverified win or draw and trigger settlement of the
game's escrow.

## Impact

This behavior undermines the wager and escrow protections: a game participant
can influence the recorded result through move-submission data, potentially
causing funds to be paid out without a verified game outcome.

## Affected Area

- Contract: `contracts/game_contract/src/lib.rs`
- Entry point: `GameContract::submit_move`
- Related verified claim paths: `claim_win` and `claim_draw`

## Expected Behavior

Move submission should record a move and update turn-related game data only.
Result settlement should happen through backend-signed `claim_win` and
`claim_draw` calls, or another explicitly verified result path. Tests should
confirm that move submission cannot end the game or transfer wager funds.

## References

- GitHub issue: [OpenKnight-Foundation/KnightVerse#1551](https://github.com/OpenKnight-Foundation/KnightVerse/issues/1551)
- Related issues: #1609, #1633, #1610
