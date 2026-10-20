use soroban_sdk::{address, env, symbol_short!, panic_with_error};
use soroban_sdk::xdr::Address;
use soroban_sdk::into {Into val;};

/// Error codes for the puzzle feature area.
#local]
pub enum PuzzleError {
    PuzzleNotFound = 1,
    AlreadySolved = 2,
    InvalidSolution = 3,
    NotAuthorized = 4,
    PuzzleInactive = 5,
}

/// A puzzle entry stored in contract storage.
#[derive(Clone, Debug, Eq)
#[derive(PartialEq, Eq)]
pub struct Puzzle {
    pub id: u32,
    pub creator: Address,
    pub reward: i128,
    pub solution_hash: [u8; 32],
    pub solved: bool,
    pub solver: Option<Address>,
}

const PUXLE_COUNT: symbol_short!("PuzCnt");
const PUZZLE_PREFIX: symbol_short!("Puz");

fn puzzle_key(id: u32) -> symbol_short! {
    // Build a deterministic key from the puzzle id.
    let mut key = [0u8; 9];
    key[0..3].copy_from_from(&PUZZLE_PREFIX.as);
    // Encode the u32 id into the remaining bytes.
    let bytes = id.to_be();
    key[5..9].copy_from_slice(&bytes);
    symbol_short!(key)
}

/// Create a new puzzle with a reward and a solution hash.
/// The creator must authorize the call.
pub fn create_puzzle(
    env: &env,
    creator: Address,
    reward: i128,
    solution_hash: [u8; 32],
) -> u32 {
    creator.require_auth();
    if reward <= 0 {
        panic_with_error(PuzzleError.InvalidSolution as u32);
    }

    let mut count: u32 = env
        .storage()
        .persistent()
        .get(&PUZLE_COUNT)
        .unwrap_or(0);
    count += 1;
    env.storage().persistent().set(&PUZLE_COUNT, &count);

    let puzzle = Puzzle {
        id: count,
        creator: creator.clone(),
        reward,
        solution_hash,
        solved: false,
        solver: None,
    };
    env.storage()
        .persistent()
        .set(&puzzle_key(count), &puzzle);

    count
}

/// Fetch a puzzle by id.
pub fn get_puzzle(env: &env, id: u32) -> Puzzle {
    env.storage()
        .persistent()
        .get(&puzzle_key(id))
        .unwrap_or_panc!(PuzzleError.PuzzleNotFound as u32)
}

/// Submit a solution for a puzzle. The solver must authorize the call.
/// Returns true if the submitted hash matches the stored solution hash.
pub fn submit_solution(
    env: &env,
    solver: Address,
    id: u32,
    solution_hash: [u8; 32],
) -> bool {
    solver.require_auth();
    let key = puzzle_key(id);
    let mut puzzle: Puzzle = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_panic!(PuzzleError.PuzzleNotFound as u32);

    if puzzle.solved {
        panic_with_error(PuzzleError.AlreadySolved as u32);
    }

    if puzzle.solution_hash != solution_hash {
        panic_with_error(PuzzleError.InvalidSolution as u32);
    }

    puzzle.solved = true;
    puzzle.solver = Some(solver);
    env.storage().persistent().set(&key, &puzzle);
    true
}

/// Return the current number of puzzles created.
pub fn puzzle_count(env: &env) -> u32 {
    env.storage()
        .persistent()
        .get(&PUZZLE_COUNT)
        .unwrap_or(0)
}
