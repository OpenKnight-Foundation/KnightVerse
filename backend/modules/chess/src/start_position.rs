//! Starting-position FEN generation per game variant (BE-133).
//!
//! Ported out of `backend/modules/src/utils/fen_generator.rs`, which lived in
//! a crate that was never a workspace member, had no `mod.rs` files, and could
//! not have compiled: its service module imported
//! `crate::services::game_service` while the file was `game_services.rs`, and
//! called `generate_fen` where the function was `generate_starting_fen`.
//!
//! The logic was sound and worth keeping — the live `dto::games` types have no
//! `variant` field at all, so nothing else produces a Chess960 start position.
//!
//! The Chess960 generator was rewritten rather than copied: the original picked
//! the two rooks and king from whatever squares were left and only checked
//! `positions[0] < positions[1] < positions[2]` *after* sorting, which does not
//! guarantee the king is between the rooks for every permutation it accepted.
//! This version enforces the Chess960 rules directly and then verifies the
//! result with shakmaty.

use serde::{Deserialize, Serialize};

/// Starting position to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GameVariant {
    #[default]
    Standard,
    Chess960,
    /// Three-check starts from the standard array; the rules engine tracks the
    /// remaining checks, so the FEN is identical to standard.
    ThreeCheck,
}

/// The standard starting array.
pub const STANDARD_START_FEN: &str =
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Generate the starting FEN for `variant`.
///
/// Chess960 uses a fresh pseudo-random back rank each call; the other variants
/// are the standard position.
pub fn generate_starting_fen(variant: GameVariant) -> String {
    match variant {
        GameVariant::Standard | GameVariant::ThreeCheck => STANDARD_START_FEN.to_string(),
        GameVariant::Chess960 => generate_chess960_fen(),
    }
}

/// Build one valid Chess960 back rank and render it as a FEN.
fn generate_chess960_fen() -> String {
    let rank = generate_back_rank();
    let back_rank: String = rank.iter().collect();

    // Castling rights are named after the *rook* files, and the h-side right is
    // always written after the a-side one (Shredder-FEN ordering: KQkq where the
    // king may sit anywhere between its rooks).
    let [a_rook, king, h_rook] = rooks_and_king(&rank);

    let mut white = String::new();
    let mut black = String::new();
    for file in [a_rook, h_rook] {
        if file < king {
            white.push((b'A' + file as u8) as char);
            black.push((b'a' + file as u8) as char);
        } else {
            black.push((b'a' + file as u8) as char);
            white.push((b'A' + file as u8) as char);
        }
    }

    format!(
        "{}/pppppppp/8/8/8/8/PPPPPPPP/{} w {} - 0 1",
        back_rank.to_lowercase(),
        back_rank,
        white + &black
    )
}

/// The file indices of the a-side rook, the king and the h-side rook.
fn rooks_and_king(rank: &[char; 8]) -> [usize; 3] {
    let king = rank.iter().position(|c| *c == 'K').unwrap_or(0);
    let rooks: Vec<usize> = rank
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == 'R')
        .map(|(i, _)| i)
        .collect();
    let (a_rook, h_rook) = if rooks.len() == 2 {
        (rooks[0], rooks[1])
    } else {
        // Unreachable for a valid rank; fall back to a legal standard-ish
        // layout rather than panicking in a request path.
        (0, 7)
    };
    [a_rook.min(h_rook), king, a_rook.max(h_rook)]
}

/// Place the eight pieces on a random, rule-valid back rank.
fn generate_back_rank() -> [char; 8] {
    use rand::seq::SliceRandom;
    use rand::thread_rng;

    let mut rng = thread_rng();
    let mut rank = [' '; 8];

    // Bishops on opposite colours.
    let light: Vec<usize> = (0..8).step_by(2).collect();
    let dark: Vec<usize> = (1..8).step_by(2).collect();
    let b_light = *light.choose(&mut rng).unwrap();
    let b_dark = *dark.choose(&mut rng).unwrap();
    rank[b_light] = 'B';
    rank[b_dark] = 'B';

    let mut free: Vec<usize> = (0..8)
        .filter(|i| *i != b_light && *i != b_dark)
        .collect();

    // Queen anywhere on a free square.
    let queen = free.choose(&mut rng).copied().unwrap();
    rank[queen] = 'Q';
    free.retain(|i| *i != queen);

    // Knights next.
    let n1 = free.choose(&mut rng).copied().unwrap();
    rank[n1] = 'N';
    free.retain(|i| *i != n1);
    let n2 = free.choose(&mut rng).copied().unwrap();
    rank[n2] = 'N';
    free.retain(|i| *i != n2);

    // Three squares remain. Put the king between the rooks: try each of the
    // three as the king and keep the first that satisfies the rule.
    let mut remaining = free.clone();
    remaining.sort_unstable();
    for king in remaining.iter().copied() {
        let mut others: Vec<usize> = remaining.iter().copied().filter(|i| *i != king).collect();
        others.sort_unstable();
        let (r1, r2) = (others[0], others[1]);
        if r1 < king && king < r2 {
            rank[king] = 'K';
            rank[r1] = 'R';
            rank[r2] = 'R';
            return rank;
        }
    }

    // Unreachable: the a-file/bishop constraint guarantees at least one of the
    // three free squares sits between the other two.
    rank[remaining[0]] = 'K';
    rank[remaining[1]] = 'R';
    rank[remaining[2]] = 'R';
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_and_three_check_share_the_starting_position() {
        assert_eq!(generate_starting_fen(GameVariant::Standard), STANDARD_START_FEN);
        assert_eq!(
            generate_starting_fen(GameVariant::ThreeCheck),
            STANDARD_START_FEN
        );
    }

    #[test]
    fn default_variant_is_standard() {
        assert_eq!(GameVariant::default(), GameVariant::Standard);
        assert_eq!(generate_starting_fen(GameVariant::default()), STANDARD_START_FEN);
    }

    #[test]
    fn chess960_fen_has_six_fields() {
        let fen = generate_starting_fen(GameVariant::Chess960);
        let parts: Vec<&str> = fen.split(' ').collect();
        assert_eq!(parts.len(), 6);
        assert_eq!(parts[1], "w");
        assert_eq!(parts[3], "-");
        assert_eq!(parts[4], "0");
        assert_eq!(parts[5], "1");
    }

    #[test]
    fn chess960_back_rank_has_the_right_piece_counts() {
        for _ in 0..200 {
            let fen = generate_starting_fen(GameVariant::Chess960);
            let rank = fen.rsplit('/').next().unwrap();

            let mut counts = std::collections::HashMap::new();
            for c in rank.chars() {
                *counts.entry(c).or_insert(0usize) += 1;
            }
            assert_eq!(counts.get(&'R'), Some(&2), "two rooks in {fen}");
            assert_eq!(counts.get(&'N'), Some(&2), "two knights in {fen}");
            assert_eq!(counts.get(&'B'), Some(&2), "two bishops in {fen}");
            assert_eq!(counts.get(&'Q'), Some(&1), "one queen in {fen}");
            assert_eq!(counts.get(&'K'), Some(&1), "one king in {fen}");
        }
    }

    #[test]
    fn chess960_bishops_are_on_opposite_colours() {
        for _ in 0..200 {
            let fen = generate_starting_fen(GameVariant::Chess960);
            let rank = fen.rsplit('/').next().unwrap();
            let bishops: Vec<usize> = rank
                .char_indices()
                .filter(|(_, c)| *c == 'B')
                .map(|(i, _)| i)
                .collect();
            assert_eq!(bishops.len(), 2);
            assert_ne!(
                bishops[0] % 2,
                bishops[1] % 2,
                "bishops on the same colour square in {fen}"
            );
        }
    }

    /// The last FEN field is the back rank; copy it into a fixed array so
    /// `rooks_and_king` can index it.
    fn back_rank_of(fen: &str) -> [char; 8] {
        let mut rank = [' '; 8];
        for (slot, c) in rank.iter_mut().zip(fen.rsplit('/').next().unwrap().chars()) {
            *slot = c;
        }
        rank
    }

    #[test]
    fn chess960_king_is_between_its_rooks() {
        for _ in 0..500 {
            let fen = generate_starting_fen(GameVariant::Chess960);
            let [a_rook, king, h_rook] = rooks_and_king(&back_rank_of(&fen));
            assert!(
                a_rook < king && king < h_rook,
                "king not between rooks in {fen}"
            );
        }
    }

    #[test]
    fn chess960_castling_rights_name_the_rook_files() {
        for _ in 0..200 {
            let fen = generate_starting_fen(GameVariant::Chess960);
            let [a_rook, _, h_rook] = rooks_and_king(&back_rank_of(&fen));

            let rights = fen.split(' ').nth(2).unwrap();
            assert_eq!(rights.len(), 4, "expected KQkq in {fen}");

            let files: Vec<usize> = rights
                .chars()
                .map(|c| (c.to_ascii_lowercase() as u8 - b'a') as usize)
                .collect();
            assert!(
                files.contains(&a_rook) && files.contains(&h_rook),
                "castling rights {rights} do not name both rook files ({a_rook}, {h_rook}) in {fen}"
            );
            assert!(
                files.iter().all(|f| *f == a_rook || *f == h_rook),
                "castling rights {rights} name a non-rook file in {fen}"
            );
        }
    }

    #[test]
    fn generated_positions_are_well_formed_for_the_engine() {
        for _ in 0..100 {
            let fen = generate_starting_fen(GameVariant::Chess960);
            // shakmaty must at least parse it as a FEN, otherwise the engine
            // will reject the position at the first move.
            fen.parse::<shakmaty::fen::Fen>()
                .unwrap_or_else(|e| panic!("shakmaty rejected {fen}: {e}"));
        }
    }
}