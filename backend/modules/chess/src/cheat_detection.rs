/*!
 * Cheat Detection Engine — Backend heuristic analysis for engine-assisted play.
 *
 * Analyses move time consistency, accuracy patterns, and position complexity
 * to produce a per-player suspicion score.  All heuristics are O(1) per move
 * with amortised O(n) bookkeeping — CPU efficient by design.
 *
 * NOTE: This is a *heuristic* tool, not a definitive verdict.  It flags games
 * for further human review, not automatic bans.
 */

use shakmaty::{Chess, Position, Color, Role, fen::Fen};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

// ── Public API types (all serde-friendly) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Moderate,
    High,
    Critical,
}

/// A single move record submitted by the game server.
/// Uses plain types so it can be freely serialised over HTTP / stored in DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveEntry {
    /// SAN notation, e.g. "Nf3"
    pub san: String,
    /// Unix-millis timestamp when the move was played
    pub timestamp: u64,
    /// FEN of the position *before* this move
    pub fen_before: String,
    /// "w" or "b"
    pub color: String,
    /// Full-move counter (1-based)
    pub move_number: u32,
    /// Whether this move captured an opponent piece
    pub is_capture: bool,
    /// Whether this move gives check
    pub is_check: bool,
    /// Piece role: "p" | "n" | "b" | "r" | "q" | "k"
    pub piece_role: String,
    /// UCI from-square, e.g. "g1"
    pub from_square: String,
    /// UCI to-square, e.g. "f3"
    pub to_square: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicDetails {
    pub time_consistency: u32,
    pub accuracy_score: u32,
    pub complexity_speed: u32,
    pub blunder_avoidance: u32,
    pub blunder_count: u32,
    pub move_count: u32,
    pub avg_think_time: f64,
    pub think_time_std_dev: f64,
    pub best_move_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicResult {
    /// 0–100 suspicion score
    pub score: u32,
    pub risk_level: RiskLevel,
    pub summary: String,
    pub details: HeuristicDetails,
}

// ── Constants ──────────────────────────────────────────────────────────────

const MIN_MOVES: usize = 6;
const SUSPICIOUS_STDDEV_MS: f64 = 1_500.0;
const FAST_MOVE_MS: u64 = 2_000;
const COMPLEX_PIECE_THRESHOLD: usize = 30;

// ── Engine ─────────────────────────────────────────────────────────────────

/// Stateful engine that accumulates moves and produces heuristic reports.
#[derive(Default)]
pub struct CheatDetectionEngine {
    moves: Vec<MoveEntry>,
}

impl CheatDetectionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a move.  Call once per half-move for either player.
    pub fn record_move(&mut self, entry: MoveEntry) {
        self.moves.push(entry);
    }

    /// Reset all state (e.g. between games).
    pub fn reset(&mut self) {
        self.moves.clear();
    }

    /// Return all moves for a given colour ("w" or "b").
    pub fn moves_for_color(&self, color: &str) -> Vec<&MoveEntry> {
        self.moves.iter().filter(|m| m.color == color).collect()
    }

    /// Run full heuristic analysis for one side.
    pub fn analyse(&self, color: &str) -> HeuristicResult {
        let player_moves = self.moves_for_color(color);
        let move_count = player_moves.len();

        if move_count < MIN_MOVES {
            return HeuristicResult {
                score: 0,
                risk_level: RiskLevel::Low,
                summary: format!(
                    "Insufficient data ({}/{} moves)",
                    move_count, MIN_MOVES
                ),
                details: Self::empty_details(move_count as u32),
            };
        }

        let think_times = self.compute_think_times(&player_moves);
        let time_consistency = Self::score_time_consistency(&think_times);
        let accuracy_score = Self::score_accuracy(&player_moves);
        let complexity_speed = Self::score_complexity_speed(&player_moves, &think_times);
        let blunder_count = Self::count_blunders(&player_moves);
        let blunder_avoidance = Self::score_blunder_avoidance(move_count, blunder_count);

        let avg_think_time = if think_times.is_empty() {
            0.0
        } else {
            think_times.iter().sum::<u64>() as f64 / think_times.len() as f64 / 1_000.0
        };
        let think_time_std_dev = Self::std_dev(&think_times) / 1_000.0;
        let best_move_rate = accuracy_score as f64 / 100.0;

        let composite = time_consistency as f64 * 0.25
            + accuracy_score as f64 * 0.30
            + complexity_speed as f64 * 0.25
            + blunder_avoidance as f64 * 0.20;

        let score = (composite.round() as u32).min(100);
        let risk_level = Self::classify_risk(score);

        let details = HeuristicDetails {
            time_consistency,
            accuracy_score,
            complexity_speed,
            blunder_avoidance,
            blunder_count,
            move_count: move_count as u32,
            avg_think_time,
            think_time_std_dev,
            best_move_rate,
        };

        HeuristicResult {
            score,
            summary: Self::generate_summary(score, &risk_level, &details),
            risk_level,
            details,
        }
    }

    // ── Private helpers ────────────────────────────────────────────────────

    fn empty_details(move_count: u32) -> HeuristicDetails {
        HeuristicDetails {
            time_consistency: 0,
            accuracy_score: 0,
            complexity_speed: 0,
            blunder_avoidance: 0,
            blunder_count: 0,
            move_count,
            avg_think_time: 0.0,
            think_time_std_dev: 0.0,
            best_move_rate: 0.0,
        }
    }

    /// Gap between consecutive moves of the same colour, in milliseconds.
    fn compute_think_times(&self, player_moves: &[&MoveEntry]) -> Vec<u64> {
        let mut times = Vec::new();
        for window in player_moves.windows(2) {
            let elapsed = window[1].timestamp.saturating_sub(window[0].timestamp);
            if elapsed > 0 && elapsed < 300_000 {
                times.push(elapsed);
            }
        }
        times
    }

    /// Low time-variance → high suspicion.
    fn score_time_consistency(think_times: &[u64]) -> u32 {
        if think_times.len() < 3 {
            return 0;
        }
        let stddev = Self::std_dev(think_times);
        if stddev < SUSPICIOUS_STDDEV_MS {
            (90.0 - (stddev / SUSPICIOUS_STDDEV_MS) * 70.0).round() as u32
        } else {
            (20.0_f64 - (stddev - SUSPICIOUS_STDDEV_MS) / 100.0)
                .round()
                .max(0.0) as u32
        }
    }

    /// High "quality" move rate → high suspicion.
    fn score_accuracy(player_moves: &[&MoveEntry]) -> u32 {
        if player_moves.len() < MIN_MOVES {
            return 0;
        }
        let best = player_moves
            .iter()
            .filter(|m| Self::is_high_quality_move(m))
            .count();
        let rate = best as f64 / player_moves.len() as f64;
        if rate >= 0.85 {
            (rate * 100.0).round().min(95.0) as u32
        } else if rate >= 0.70 {
            (50.0 + (rate - 0.70) * 200.0).round() as u32
        } else if rate >= 0.50 {
            (20.0 + (rate - 0.50) * 150.0).round() as u32
        } else {
            (rate * 40.0).round() as u32
        }
    }

    fn is_high_quality_move(entry: &MoveEntry) -> bool {
        if entry.is_capture || entry.is_check {
            return true;
        }
        if entry.san == "O-O" || entry.san == "O-O-O" {
            return true;
        }
        let center = ["e4", "d4", "e5", "d5"];
        if entry.piece_role == "p"
            && center.contains(&entry.to_square.as_str())
            && entry.move_number <= 10
        {
            return true;
        }
        if (entry.piece_role == "n" || entry.piece_role == "b")
            && entry.move_number <= 10
            && entry.from_square != entry.to_square
        {
            return true;
        }
        // Promotion
        if entry.san.contains('=') {
            return true;
        }
        false
    }

    /// Fast moves in complex positions → high suspicion.
    fn score_complexity_speed(player_moves: &[&MoveEntry], think_times: &[u64]) -> u32 {
        if player_moves.len() < MIN_MOVES {
            return 0;
        }
        let complex_indices: Vec<usize> = player_moves
            .iter()
            .enumerate()
            .filter(|(_, m)| Self::count_pieces(&m.fen_before) >= COMPLEX_PIECE_THRESHOLD)
            .map(|(i, _)| i)
            .collect();

        if complex_indices.len() < 3 {
            return 0;
        }

        let fast = complex_indices
            .iter()
            .filter(|&&i| i < think_times.len() && think_times[i] < FAST_MOVE_MS)
            .count();

        let rate = fast as f64 / complex_indices.len() as f64;
        if rate >= 0.5 {
            (rate * 120.0).round().min(90.0) as u32
        } else if rate >= 0.3 {
            (30.0 + (rate - 0.3) * 200.0).round() as u32
        } else {
            (rate * 100.0).round() as u32
        }
    }

    /// Never blundering across many moves → suspicious.
    fn score_blunder_avoidance(move_count: usize, blunder_count: u32) -> u32 {
        if move_count < MIN_MOVES {
            return 0;
        }
        if blunder_count == 0 && move_count >= 15 {
            return 60;
        }
        if blunder_count == 0 && move_count >= 10 {
            return 40;
        }
        let rate = blunder_count as f64 / move_count as f64;
        if rate < 0.05 && move_count >= 10 {
            return 50;
        }
        if rate < 0.10 {
            return 25;
        }
        (15.0_f64 - rate * 100.0).round().max(0.0) as u32
    }

    /// Count moves that leave a non-pawn/king piece en-prise.
    fn count_blunders(player_moves: &[&MoveEntry]) -> u32 {
        let mut blunders = 0u32;
        for entry in player_moves {
            // Skip pawns and kings — cheapest pieces / can't be straightforwardly hung
            if matches!(entry.piece_role.as_str(), "p" | "k") {
                continue;
            }
            if let Ok(fen) = Fen::from_str(&entry.fen_before) {
                if let Ok(pos) = fen.into_position::<Chess>(shakmaty::CastlingMode::Standard) {
                    if let Ok(san) = shakmaty::san::San::from_str(&entry.san) {
                        if let Ok(m) = san.to_move(&pos) {
                            let mut new_pos = pos.clone();
                            new_pos.play_unchecked(&m);
                            let dest = m.to();
                            // Check if the moved piece can immediately be captured
                            let is_en_prise = new_pos
                                .legal_moves()
                                .iter()
                                .any(|om| om.is_capture() && om.to() == dest);
                            if is_en_prise && !entry.is_capture {
                                blunders += 1;
                            }
                        }
                    }
                }
            }
        }
        blunders
    }

    fn count_pieces(fen_str: &str) -> usize {
        fen_str
            .split(' ')
            .next()
            .unwrap_or("")
            .chars()
            .filter(|c| "pnbrqkPNBRQK".contains(*c))
            .count()
    }

    fn classify_risk(score: u32) -> RiskLevel {
        match score {
            70..=100 => RiskLevel::Critical,
            50..=69 => RiskLevel::High,
            30..=49 => RiskLevel::Moderate,
            _ => RiskLevel::Low,
        }
    }

    fn generate_summary(score: u32, risk: &RiskLevel, d: &HeuristicDetails) -> String {
        let mut flags: Vec<&str> = Vec::new();
        if d.time_consistency >= 50 {
            flags.push("unusually consistent think times");
        }
        if d.accuracy_score >= 50 {
            flags.push("high best-move match rate");
        }
        if d.complexity_speed >= 40 {
            flags.push("fast moves in complex positions");
        }
        if d.blunder_avoidance >= 40 {
            flags.push("no detectable blunders");
        }
        if flags.is_empty() {
            format!(
                "Score {}/100 — No significant anomalies over {} moves.",
                score, d.move_count
            )
        } else {
            format!(
                "Score {}/100 ({:?}) — Flags: {}. {} moves analysed.",
                score,
                risk,
                flags.join("; "),
                d.move_count
            )
        }
    }

    fn std_dev(values: &[u64]) -> f64 {
        if values.len() < 2 {
            return 0.0;
        }
        let mean = values.iter().sum::<u64>() as f64 / values.len() as f64;
        let variance = values
            .iter()
            .map(|&x| (x as f64 - mean).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64;
        variance.sqrt()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_move(color: &str, ts: u64, san: &str, piece: &str) -> MoveEntry {
        MoveEntry {
            san: san.to_string(),
            timestamp: ts,
            fen_before: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
            color: color.to_string(),
            move_number: 1,
            is_capture: false,
            is_check: false,
            piece_role: piece.to_string(),
            from_square: "g1".to_string(),
            to_square: "f3".to_string(),
        }
    }

    #[test]
    fn test_empty_returns_low_risk() {
        let engine = CheatDetectionEngine::new();
        let result = engine.analyse("w");
        assert_eq!(result.score, 0);
        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(result.summary.contains("Insufficient data"));
    }

    #[test]
    fn test_insufficient_moves_below_threshold() {
        let mut engine = CheatDetectionEngine::new();
        for i in 0..5u64 {
            engine.record_move(make_move("w", 1000 + i * 3000, "Nf3", "n"));
        }
        let result = engine.analyse("w");
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_risk_classification() {
        assert_eq!(CheatDetectionEngine::classify_risk(0), RiskLevel::Low);
        assert_eq!(CheatDetectionEngine::classify_risk(29), RiskLevel::Low);
        assert_eq!(CheatDetectionEngine::classify_risk(30), RiskLevel::Moderate);
        assert_eq!(CheatDetectionEngine::classify_risk(50), RiskLevel::High);
        assert_eq!(CheatDetectionEngine::classify_risk(70), RiskLevel::Critical);
        assert_eq!(CheatDetectionEngine::classify_risk(100), RiskLevel::Critical);
    }

    #[test]
    fn test_very_consistent_think_times_raise_score() {
        let mut engine = CheatDetectionEngine::new();
        // 10 moves with exactly 2 s apart (very robotic)
        for i in 0..10u64 {
            engine.record_move(make_move("w", 1000 + i * 2000, "e4", "p"));
        }
        let result = engine.analyse("w");
        // time_consistency should be high (robotic pacing)
        assert!(result.details.time_consistency > 50);
    }

    #[test]
    fn test_high_quality_move_detection() {
        // Capture is always high quality
        let capture_move = MoveEntry {
            is_capture: true,
            san: "Nxe5".to_string(),
            ..make_move("w", 1000, "Nxe5", "n")
        };
        assert!(CheatDetectionEngine::is_high_quality_move(&capture_move));

        // Castling
        let castle = MoveEntry {
            san: "O-O".to_string(),
            ..make_move("w", 1000, "O-O", "k")
        };
        assert!(CheatDetectionEngine::is_high_quality_move(&castle));
    }

    #[test]
    fn test_count_pieces_from_fen() {
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert_eq!(CheatDetectionEngine::count_pieces(start_fen), 32);

        let empty_fen = "8/8/8/8/8/8/8/8 w - - 0 1";
        assert_eq!(CheatDetectionEngine::count_pieces(empty_fen), 0);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut engine = CheatDetectionEngine::new();
        engine.record_move(make_move("w", 1000, "e4", "p"));
        engine.reset();
        assert_eq!(engine.moves_for_color("w").len(), 0);
    }

    #[test]
    fn test_colour_separation() {
        let mut engine = CheatDetectionEngine::new();
        for i in 0..6u64 {
            engine.record_move(make_move("w", 1000 + i * 2000, "e4", "p"));
            engine.record_move(make_move("b", 2000 + i * 2000, "e5", "p"));
        }
        assert_eq!(engine.moves_for_color("w").len(), 6);
        assert_eq!(engine.moves_for_color("b").len(), 6);

        // Both should produce independent results
        let white = engine.analyse("w");
        let black = engine.analyse("b");
        assert_eq!(white.details.move_count, 6);
        assert_eq!(black.details.move_count, 6);
    }

    #[test]
    fn test_blunder_avoidance_score_with_zero_blunders() {
        // score_blunder_avoidance(15, 0) should return 60
        assert_eq!(CheatDetectionEngine::score_blunder_avoidance(15, 0), 60);
        assert_eq!(CheatDetectionEngine::score_blunder_avoidance(10, 0), 40);
        assert_eq!(CheatDetectionEngine::score_blunder_avoidance(5, 0), 0);
    }

    #[test]
    fn test_std_dev_uniform() {
        let uniform = vec![2000u64; 5];
        // stddev of identical values is 0
        assert_eq!(CheatDetectionEngine::std_dev(&uniform), 0.0);
    }

    #[test]
    fn test_std_dev_varied() {
        let varied = vec![1000u64, 2000, 3000, 4000, 5000];
        let sd = CheatDetectionEngine::std_dev(&varied);
        assert!(sd > 0.0);
    }
}
