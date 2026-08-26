# SC-13: Comprehensive Event Emission Implementation for Soroban Contracts

**Status:** Complete  
**Date:** 2026-07-29  
**Task:** Emit explicit Soroban events on all major lifecycle actions for efficient subgraph indexing

---

## Overview

This implementation adds comprehensive event emissions across the KnightVerse Soroban contracts to enable efficient indexing by subgraphs and external services. Events are now emitted on all major lifecycle actions including:

- **AI NFT Contract**: NFT minting and transfers
- **Game Contract**: Tournament creation, joining, moves, payouts, forfeits, draws, and escrow management

---

## Changes Made

### 1. AI NFT Contract (`contracts/ai_nft/src/lib.rs`)

#### NFT Mint Event
**Location:** `mint()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("ai_nft"), symbol_short!("mint")),
    (nft_counter, minter, metadata_hash),
);
```
**Payload:**
- `nft_id`: u64 - The newly minted NFT ID
- `minter`: Address - The address that minted the NFT
- `metadata_hash`: BytesN<32> - IPFS/content hash of NFT metadata

**Timing:** After successful mint when NFT is stored

#### NFT Transfer Event
**Location:** `transfer()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("ai_nft"), symbol_short!("transfer")),
    (nft_id, current_owner, to),
);
```
**Payload:**
- `nft_id`: u64 - The NFT being transferred
- `from`: Address - Previous owner
- `to`: Address - New owner

**Timing:** After successful ownership transfer

---

### 2. Game Contract (`contracts/game_contract/src/lib.rs`)

#### Game Created Event
**Location:** `create_game()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("created")),
    (game_counter, player1, wager_amount),
);
```
**Payload:**
- `game_id`: u64 - Newly created game ID
- `player1`: Address - Game creator (first player)
- `wager_amount`: i128 - Token amount wagered

**Timing:** After game is stored and escrow is updated

#### Game Joined Event
**Location:** `join_game()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("joined")),
    (game_id, game.player1, player2),
);
```
**Payload:**
- `game_id`: u64 - Game that was joined
- `player1`: Address - Initial player
- `player2`: Address - Player joining the game

**Timing:** After game transitions to InProgress state

#### Draw Claimed Event
**Location:** `claim_draw()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("drawn")),
    (game_id, player),
);
```
**Payload:**
- `game_id`: u64 - Game that ended in draw
- `player`: Address - Player claiming the draw

**Timing:** After draw payout is processed

#### Win Claimed Event
**Location:** `claim_win()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("won")),
    (game_id, winner),
);
```
**Payload:**
- `game_id`: u64 - Game with winner
- `winner`: Address - Winning player

**Timing:** After winner payout is processed

#### Game Forfeited Event
**Location:** `forfeit()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("forfeited")),
    (game_id, player, winner),
);
```
**Payload:**
- `game_id`: u64 - Game being forfeited
- `forfeiting_player`: Address - Player forfeit the game
- `winner`: Address - Player receiving win payout

**Timing:** After forfeit payout is processed and winner is determined

#### Game Cancelled Event
**Location:** `cancel_game()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("cancelled")),
    (game_id, player),
);
```
**Payload:**
- `game_id`: u64 - Game being cancelled
- `player`: Address - Player who cancelled (must be player1)

**Timing:** After refund is processed and game state updated

#### Standard Payout Event
**Location:** `payout()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("payout")),
    (game_id, winner),
);
```
**Payload:**
- `game_id`: u64 - Game settling payout
- `winner`: Address - Player receiving payout

**Timing:** After payout is processed and game transitions to Settled

#### Tournament Payout Event
**Location:** `payout_tournament()` function  
**Event Structure:**
```rust
env.events().publish(
    (symbol_short!("game"), symbol_short!("tournament_payout")),
    (game_id, winners.len() as u32),
);
```
**Payload:**
- `game_id`: u64 - Tournament game distributing prizes
- `winner_count`: u32 - Number of winners in tournament

**Timing:** After all payouts are distributed and game is settled

#### Tournament Escrow Created Event
**Location:** `create_tournament_escrow()` function  
**Event Structure:** *(Already implemented)*
```rust
env.events().publish(
    (symbol_short!("tl_escrow"), symbol_short!("created")),
    (escrow_id, game_id, locked_until),
);
```

#### Tournament Escrow Released Event
**Location:** `release_tournament_escrow()` function  
**Event Structure:** *(Already implemented)*
```rust
env.events().publish(
    (symbol_short!("tl_escrow"), symbol_short!("released")),
    escrow_id,
);
```

---

## Event Topics Summary

### AI NFT Contract Events
| Topic | Subtopic | Event Name | Use Case |
|-------|----------|-----------|----------|
| `ai_nft` | `mint` | NFT Minted | Track new AI NFT creation |
| `ai_nft` | `transfer` | NFT Transferred | Track NFT ownership changes |

### Game Contract Events
| Topic | Subtopic | Event Name | Use Case |
|-------|----------|-----------|----------|
| `game` | `created` | Tournament Created | Index new tournaments |
| `game` | `joined` | Tournament Joined | Track when tournaments begin |
| `game` | `won` | Game Won | Track match results |
| `game` | `drawn` | Game Drawn | Track draw outcomes |
| `game` | `forfeited` | Game Forfeited | Track forfeit outcomes |
| `game` | `cancelled` | Game Cancelled | Track cancelled games |
| `game` | `payout` | Payout Processed | Track reward distribution |
| `game` | `tournament_payout` | Tournament Payout | Track multi-winner distributions |
| `tl_escrow` | `created` | Escrow Created | Track prize pool locks |
| `tl_escrow` | `released` | Escrow Released | Track prize distribution releases |

---

## Indexing Benefits

These events enable subgraph indexers to efficiently:

1. **Track Tournament Lifecycle**: From creation through join, settlement, and payout
2. **Monitor AI NFT Activity**: Mint events enable tracking of NFT creation and ownership transfers
3. **Audit Financial Flows**: Payout events provide complete history of reward distribution
4. **Enforce Game State**: Events mark state transitions (Created → InProgress → Settled)
5. **Detect Disputes**: Know when games end (win, draw, forfeit, cancel) for dispute resolution
6. **Time-lock Verification**: Escrow creation and release events track prize pool locks

---

## Implementation Details

### Event Emission Pattern
All events follow the Soroban SDK pattern:
```rust
env.events().publish(
    (primary_topic: Symbol, secondary_topic: Symbol),
    payload: impl IntoVal<Env, Val>,
);
```

### Two-Level Topic System
- **Primary Topic**: Contract or feature domain (e.g., `game`, `ai_nft`, `tl_escrow`)
- **Secondary Topic**: Action or event type (e.g., `created`, `mint`, `payout`)

### Payload Flexibility
Payloads are emitted as tuples, allowing indexers to extract multiple data points per event:
- Addresses (players, minters, winners)
- IDs (game_id, nft_id, escrow_id)
- Amounts (wagers, percentages)
- Counts (winner count in tournaments)

---

## Testing Recommendations

### Unit Tests to Add
1. **Verify events are published** in create_game, join_game, claim_win, etc.
2. **Check event topics** match expected symbols
3. **Validate event payloads** contain correct addresses and IDs
4. **Test event ordering** (e.g., payout event fires after state update)

### Integration Tests
1. **End-to-end tournament flow** should emit: created → joined → won → payout
2. **Dispute scenarios** should emit: filed → resolved + payout event
3. **Timeout claims** should emit: timeout event + payout event
4. **Escrow lifecycle** should emit: created → released

### Subgraph Testing
1. Deploy events to test network
2. Verify subgraph indexes all event topics correctly
3. Confirm indexer catches all lifecycle state transitions
4. Test filtering by game_id, player address, NFT ID

---

## Files Modified

### AI NFT Contract
- **File**: `contracts/ai_nft/src/lib.rs`
- **Changes**: 
  - Added `ai_nft::mint` event in `mint()` function
  - Added `ai_nft::transfer` event in `transfer()` function

### Game Contract
- **File**: `contracts/game_contract/src/lib.rs`
- **Changes**:
  - Added `game::created` event in `create_game()` function
  - Added `game::joined` event in `join_game()` function
  - Added `game::drawn` event in `claim_draw()` function
  - Added `game::won` event in `claim_win()` function
  - Added `game::cancelled` event in `cancel_game()` function
  - Added `game::forfeited` event in `forfeit()` function
  - Added `game::payout` event in `payout()` function
  - Added `game::tournament_payout` event in `payout_tournament()` function
  - Existing `tl_escrow::created` and `tl_escrow::released` events retained

---

## Acceptance Criteria - Met ✓

- ✓ Emit explicit Soroban events on all major lifecycle actions
- ✓ Tournament creation events (`game::created`)
- ✓ AI NFT mint events (`ai_nft::mint`)
- ✓ Escrow created events (`tl_escrow::created`)
- ✓ Payout events (`game::payout`, `game::tournament_payout`)
- ✓ Additional state transition events for complete tracking:
  - Game joined, won, drawn, forfeited, cancelled
  - NFT transfers
  - Escrow releases
- ✓ Events enable efficient subgraph/indexer tracking
- ✓ Two-level topic structure for easy filtering
- ✓ Comprehensive payloads for context-rich indexing

---

## Migration Notes

### No Breaking Changes
- All event emissions are additive
- Existing contract functionality unchanged
- No storage schema modifications
- No migration required for existing data

### Backwards Compatibility
- Old games and NFTs won't have events in their history
- Indexers should handle events starting from upgrade block
- Suggest starting subgraph indexing after contract upgrade deployment

---

## Next Steps

1. **Deploy to testnet** and verify events appear in transaction logs
2. **Build subgraph indexer** to consume these event topics
3. **Create indexing schema** for efficient queries by game_id, player, NFT_id
4. **Set up event filtering** in Soroban RPC for performance
5. **Monitor production** for event consistency and completeness

---

## References

- Soroban Events: https://soroban.stellar.org/docs/learn/events
- Event Topics & Publishing: https://soroban.stellar.org/docs/build/contract-events
- Subgraph Best Practices: https://thegraph.com/docs/en/developing/creating-a-subgraph/
