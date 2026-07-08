# Game Contract Deployment Guide

## Overview
This document describes how to deploy the KnightVerse Game Contract to Stellar Testnet.

## Contract Features
- **Game Creation**: Create new chess games with XLM wagers
- **Game Joining**: Join existing games as second player
- **Move Submission**: Submit chess moves with turn validation
- **Draw Claims**: Claim draws when game is in draw state
- **Forfeiture**: Forfeit games (opponent wins)
- **Payout System**: Automatic XLM escrow and payout distribution
- **Game State Management**: Complete game lifecycle tracking

## Contract Functions

### Core Functions
- `create_game(env, player1: Address, wager_amount: i128) -> u64`
- `join_game(env, game_id: u64, player2: Address) -> Result<(), ContractError>`
- `submit_move(env, game_id: u64, player: Address, move_data: Vec<u32>) -> Result<(), ContractError>`
- `claim_draw(env, game_id: u64, player: Address) -> Result<(), ContractError>`
- `forfeit(env, game_id: u64, player: Address) -> Result<(), ContractError>`
- `payout(env, game_id: u64, winner: Address) -> Result<(), ContractError>`

### Query Functions
- `get_game(env, game_id: u64) -> Result<Game, ContractError>`
- `get_all_games(env) -> Map<u64, Game>`

## Data Structures

### Game
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

### GameState
- `Created`: Game created, waiting for second player
- `InProgress`: Game active, players can submit moves
- `Completed`: Game finished with winner
- `Drawn`: Game ended in draw
- `Forfeited`: Player forfeited

## Prerequisites
1. Install Soroban CLI
2. Have Stellar Testnet account with XLM
3. Configure network settings

## Deployment Steps

### 1. Build Contract
```bash
cd contracts
cargo build --release --target wasm32-unknown-unknown
```

### 2. Install Soroban CLI (if not installed)
```bash
cargo install --locked soroban-cli
```

### 3. Configure Network
```bash
soroban config network --global testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"
```

### 4. Deploy Contract
```bash
# Deploy to testnet
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/game_contract.wasm \
  --source <YOUR_SECRET_KEY> \
  --network testnet
```

### 5. Verify Deployment
```bash
# Get contract ID
soroban contract install \
  --wasm target/wasm32-unknown-unknown/release/game_contract.wasm \
  --source <YOUR_SECRET_KEY> \
  --network testnet
```

## Usage Examples

### Create a Game
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <PLAYER1_SECRET> \
  --network testnet \
  --create_game \
  --player1 <PLAYER1_ADDRESS> \
  --wager-amount 10000000  # 0.1 XLM (in stroops)
```

### Join a Game
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <PLAYER2_SECRET> \
  --network testnet \
  --join_game \
  --game-id 1 \
  --player2 <PLAYER2_ADDRESS>
```

### Submit a Move
```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source <PLAYER_SECRET> \
  --network testnet \
  --submit_move \
  --game-id 1 \
  --player <PLAYER_ADDRESS> \
  --move-data "[1,2,3,4]"  # Serialized chess move
```

## Security Considerations
- All XLM wagers are held in escrow during gameplay
- Turn validation prevents unauthorized moves
- Only game participants can modify game state
- Automatic payout distribution on game completion
- Draw and forfeit options for game termination

## Testing
The contract includes comprehensive test coverage for all major functions:
- Game creation and joining
- Move submission and validation
- Draw claims and forfeitures
- Payout processing
- Error handling

## Next Steps
1. Deploy to Stellar Testnet
2. Integrate with frontend application
3. Implement chess move validation logic
4. Add tournament support
5. Deploy to Mainnet after thorough testing
