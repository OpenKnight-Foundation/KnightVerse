# Swiss Pairing System

A professional implementation of the Swiss pairing system for chess tournaments, following the Dutch System specifications.

## Features

- **Dutch System Implementation**: Score-based grouping with rating tie-breaking
- **Color Balance**: Intelligent color assignment to ensure fair color distribution
- **Bye Handling**: Proper bye assignment for odd number of players (1 point to lowest ranked)
- **Floater Tracking**: Tracks players paired up/down score groups
- **Repeat Prevention**: Ensures players don't face the same opponent twice
- **Comprehensive Testing**: Full test suite with example tournament scenarios

## KnightVerse API

The authenticated API exposes persisted tournament operations:

- `GET /api/v1/tournaments` lists tournaments.
- `POST /api/v1/tournaments` creates a tournament with a `name` and optional
  `format` (`single_elimination`, `double_elimination`, or `round_robin`).
- `POST /api/v1/tournaments/{id}/register` registers the authenticated player.

The database prevents a player from registering twice for the same tournament.
Once at least two players have registered, the API generates and persists the
bracket using this crate's `BracketService`.

## Core Components

### Player
- Tracks rating, score, color history, and opponents
- Manages color balance preferences
- Handles float scores for up/down pairings

### TournamentState
- Maintains complete tournament state
- Tracks round progression
- Manages player data and pairings

### SwissPairer
- Implements Dutch System pairing algorithm
- Handles score group creation and pairing
- Manages bye assignments and floaters

## Usage Example

```rust
use tournament::{Player, TournamentState, SwissPairer, SwissConfig};
use uuid::Uuid;

// Create players
let players = vec![
    Player::new(Uuid::new_v4(), "Alice".to_string(), 2000),
    Player::new(Uuid::new_v4(), "Bob".to_string(), 1900),
    Player::new(Uuid::new_v4(), "Charlie".to_string(), 1800),
];

// Initialize tournament
let mut tournament = TournamentState::new(players, 5);
let pairer = SwissPairer::new(SwissConfig::default());

// Generate pairings for current round
let pairings = pairer.pair_round(&mut tournament)?;

// Apply round results
let results = vec![
    (player_id_1, GameResult::Win),
    (player_id_2, GameResult::Loss),
];
tournament.apply_round_results(results);
```

## Algorithm Details

### Dutch System
1. **Score Grouping**: Players grouped by current score
2. **Rating Sorting**: Within groups, sorted by rating (highest first)
3. **Color Preference**: Players with color imbalance get preferred colors
4. **Floaters**: When necessary, players float to adjacent score groups

### Bye Assignment
- Lowest ranked player (by score, then rating) receives bye
- Bye awards 1 point
- Players with previous bye are avoided when possible

### Color Balance
- Tracks white/black game history
- Assigns colors to balance preferences
- Higher rated player gets white when preferences are equal

## Testing

Run tests with:
```bash
cargo test
```

The test suite includes:
- Basic functionality tests
- Color balance verification
- Bye assignment validation
- Repeat pairing prevention
- Complete tournament scenario simulation

## Configuration

`SwissConfig` allows customization:
- `total_rounds`: Number of tournament rounds
- `rating_importance`: Weight for rating in tie-breaking
- `color_balance_weight`: Importance of color balance

## Error Handling

The system provides comprehensive error handling:
- `NoValidByeCandidate`: No eligible player for bye
- `CannotPairRemainingPlayers`: Unable to pair remaining players
- `InsufficientPlayers`: Not enough players for pairing
- `InvalidTournamentState`: Tournament state inconsistencies
