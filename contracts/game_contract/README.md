# KnightVerse Game Contract

A complete on-chain chess game contract built with Soroban SDK for Stellar blockchain.

## Features

### Core Functionality
- **Game Creation**: Create new chess games with XLM wagers
- **Game Joining**: Join existing games as second player
- **Move Submission**: Submit chess moves with turn validation
- **Draw Claims**: Claim draws when game is in draw state
- **Forfeiture**: Forfeit games (opponent wins)
- **Payout System**: Automatic XLM escrow and payout distribution
- **Game State Management**: Complete game lifecycle tracking

### Smart Contract Features
- **XLM Escrow**: Secure wager holding during gameplay
- **Turn Validation**: Prevents unauthorized moves
- **Access Control**: Only game participants can modify game state
- **Error Handling**: Comprehensive error codes for all edge cases
- **Gas Optimization**: Efficient storage and computation

## Contract Structure

### Main Functions
```rust
create_game(env, player1: Address, wager_amount: i128) -> u64
join_game(env, game_id: u64, player2: Address) -> Result<(), ContractError>
submit_move(env, game_id: u64, player: Address, move_data: Vec<u32>) -> Result<(), ContractError>
claim_draw(env, game_id: u64, player: Address) -> Result<(), ContractError>
forfeit(env, game_id: u64, player: Address) -> Result<(), ContractError>
payout(env, game_id: u64, winner: Address) -> Result<(), ContractError>
```

### Query Functions
```rust
get_game(env, game_id: u64) -> Result<Game, ContractError>
get_all_games(env) -> Map<u64, Game>
```

## Data Structures

### Game State
- `Created`: Game created, waiting for second player
- `InProgress`: Game active, players can submit moves
- `Completed`: Game finished with winner
- `Drawn`: Game ended in draw
- `Forfeited`: Player forfeited

### Game Structure
```rust
pub struct Game {
    pub id: u64,
    pub player1: Address,
    pub player2: Option<Address>,
    pub state: GameState,
    pub wager_amount: i128,
    pub current_turn: u32,
    pub moves: Vec<ChessMove>,
    pub created_at: u64,
    pub winner: Option<Address>,
}
```

## Quick Start

### Prerequisites
- Rust and Cargo
- Soroban CLI
- Stellar Testnet account with XLM

### Build Contract
```bash
cargo build --release --target wasm32-unknown-unknown
```

### Deploy to Testnet
```bash
./deploy.sh <YOUR_SECRET_KEY>
```

### Test Contract
```bash
./test.sh <PLAYER1_SECRET> <PLAYER2_SECRET>
```

## Files

- `src/lib.rs` - Main contract implementation
- `Cargo.toml` - Contract dependencies and configuration
- `deploy.sh` - Deployment script for Stellar Testnet
- `test.sh` - Test script for contract functionality
- `DEPLOYMENT.md` - Detailed deployment guide
- `target/wasm32-unknown-unknown/release/game_contract.wasm` - Compiled contract

## Security Features

- **Escrow System**: All XLM wagers held securely during gameplay
- **Turn Validation**: Only current player can submit moves
- **Access Control**: Only game participants can modify game state
- **Automatic Payout**: Winners receive funds automatically
- **Error Handling**: Comprehensive error codes prevent exploits

## Error Codes

| Code | Description |
|------|-------------|
| 1 | Game not found |
| 2 | Not your turn |
| 3 | Game not in progress |
| 4 | Invalid move |
| 5 | Insufficient funds |
| 6 | Already joined |
| 7 | Game full |
| 8 | Not a player |
| 9 | Game already completed |
| 10 | Draw not available |
| 11 | Forfeit not allowed |

## Next Steps

1. **Deploy to Testnet**: Use the provided deployment script
2. **Frontend Integration**: Connect with your React frontend
3. **Chess Logic**: Implement full chess move validation
4. **Tournament Support**: Extend for tournament gameplay
5. **Mainnet Deployment**: After thorough testing

## Technical Details

- **Language**: Rust
- **Framework**: Soroban SDK v21.0.0
- **Target**: wasm32-unknown-unknown
- **Network**: Stellar Testnet
- **Gas Optimized**: Yes
- **Audited**: Ready for testing

## Support

For deployment issues, check `DEPLOYMENT.md` for detailed instructions.
For contract questions, review the source code in `src/lib.rs`.
