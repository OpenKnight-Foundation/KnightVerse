//! Fuzz test for `validate_fen_legality`.
//!
//! Acceptance criteria (issue #1000 / BE-34): zero server panics or
//! unhandled `unwrap` crashes on fuzz-generated invalid FEN strings. This
//! throws a large, deterministic (fixed-seed, so CI is reproducible rather
//! than occasionally flaky) mix of pure-random garbage and mutated
//! near-valid FENs at the validator and asserts only that it never panics —
//! whether a given string is accepted or rejected is exercised separately
//! by the unit tests in `fen_validator.rs`.

use chess::validate_fen_legality;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::Instant;

const SEED: u64 = 0xFEED_5EED_5EED_5EEDu64;
const RANDOM_ITERATIONS: usize = 5_000;
const MUTATION_ITERATIONS: usize = 5_000;

const SEED_FENS: &[&str] = &[
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3",
    "8/8/8/4k3/8/8/4P3/4K3 w - - 0 1",
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",
];

/// Random printable-ish bytes, including characters common in FEN (digits,
/// piece letters, slash, space, dash) but with no structural guarantees at
/// all — this is the "completely garbage" half of the fuzz corpus.
fn random_fen_like_string(rng: &mut StdRng) -> String {
    const ALPHABET: &[u8] = b"pnbrqkPNBRQK12345678/ w b KQkqabcdefgh-\0\t\n\"'\\<>{}[]~";
    let len = rng.gen_range(0..=80);
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect()
}

/// Take a real, valid FEN and randomly mutate it (delete/insert/replace a
/// handful of characters). This targets the "almost legal" edge cases that
/// pure noise is unlikely to hit — off-by-one rank counts, dangling
/// castling flags, corrupted en-passant squares, etc.
fn mutate_fen(rng: &mut StdRng, base: &str) -> String {
    let mut chars: Vec<char> = base.chars().collect();
    let mutations = rng.gen_range(1..=4);

    for _ in 0..mutations {
        if chars.is_empty() {
            break;
        }
        match rng.gen_range(0..3) {
            0 => {
                // Replace a random character with a random byte from a
                // small "plausible but wrong" alphabet.
                let idx = rng.gen_range(0..chars.len());
                const REPLACEMENTS: &[char] = &[
                    'p', 'n', 'b', 'r', 'q', 'k', 'P', 'N', 'B', 'R', 'Q', 'K', '/', ' ', '-', '9',
                    '0', 'x', 'w', 'b', 'W', 'B',
                ];
                chars[idx] = REPLACEMENTS[rng.gen_range(0..REPLACEMENTS.len())];
            }
            1 => {
                // Delete a random character.
                let idx = rng.gen_range(0..chars.len());
                chars.remove(idx);
            }
            _ => {
                // Insert a random character.
                let idx = rng.gen_range(0..=chars.len());
                const INSERTS: &[char] = &['p', 'K', '/', ' ', '8', '-', 'z'];
                chars.insert(idx, INSERTS[rng.gen_range(0..INSERTS.len())]);
            }
        }
    }

    chars.into_iter().collect()
}

#[test]
fn validate_fen_legality_never_panics_on_fuzzed_input() {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut failures: Vec<String> = Vec::new();

    let start = Instant::now();

    for _ in 0..RANDOM_ITERATIONS {
        let candidate = random_fen_like_string(&mut rng);
        let result = catch_unwind(AssertUnwindSafe(|| validate_fen_legality(&candidate)));
        if result.is_err() {
            failures.push(candidate);
        }
    }

    for _ in 0..MUTATION_ITERATIONS {
        let base = SEED_FENS[rng.gen_range(0..SEED_FENS.len())];
        let candidate = mutate_fen(&mut rng, base);
        let result = catch_unwind(AssertUnwindSafe(|| validate_fen_legality(&candidate)));
        if result.is_err() {
            failures.push(candidate);
        }
    }

    let elapsed = start.elapsed();

    assert!(
        failures.is_empty(),
        "validate_fen_legality panicked on {} of {} fuzzed inputs, e.g. {:?}",
        failures.len(),
        RANDOM_ITERATIONS + MUTATION_ITERATIONS,
        &failures[..failures.len().min(5)],
    );

    // Generous ceiling (not a tight benchmark) just to catch an accidental
    // quadratic blowup; CI hardware varies a lot, so this is not a tight
    // performance assertion.
    assert!(
        elapsed.as_secs() < 10,
        "fuzzing {} inputs took {:?}, which is suspiciously slow",
        RANDOM_ITERATIONS + MUTATION_ITERATIONS,
        elapsed
    );
}

#[test]
fn validate_fen_legality_never_panics_on_pathological_strings() {
    let pathological: &[&str] = &[
        "",
        " ",
        "/",
        "////////",
        "8/8/8/8/8/8/8/8",
        "8/8/8/8/8/8/8/8 w - - 0 1",
        &"p".repeat(10_000),
        &"/".repeat(10_000),
        &"8/".repeat(1_000),
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e9 0 1",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w -------- 0 1",
        "\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}",
        "🎉🎉🎉🎉🎉🎉🎉🎉/8/8/8/8/8/8/8 w - - 0 1",
        "RNBQKBNR/PPPPPPPP/8/8/8/8/pppppppp/rnbqkbnr w KQkq - 0 1",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - -1 1",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 99999999999999999999 1",
    ];

    for fen in pathological {
        let result = catch_unwind(AssertUnwindSafe(|| validate_fen_legality(fen)));
        assert!(result.is_ok(), "validate_fen_legality panicked on {fen:?}");
    }
}
