//! Move-ordering guard for the WebSocket move stream (BE-135).
//!
//! The `move_protocol` crate has been in the workspace since BE-77, compiled in
//! CI, and never called: moves were fanned out to the opponent and to
//! spectators in whatever order the network happened to deliver them. On a
//! high-latency mobile connection that regularly replays a game with moves
//! transposed, which is indistinguishable from cheating.
//!
//! [`MoveOrderGuard`] wraps the crate's [`MoveSequencer`] with the decisions a
//! WebSocket session actually has to make:
//!
//! * **Clients that send `seq`** get true out-of-order resolution — early
//!   packets are buffered until the gap in front of them is filled, and a
//!   still-open gap after the reorder window asks for a retransmit.
//! * **Clients that don't send `seq`** keep working exactly as before. The
//!   field is `#[serde(default)]`, so adding it was backward compatible, and
//!   `None` short-circuits to "apply immediately" rather than guessing at an
//!   order the client never claimed.
//!
//! One guard is held per session, keyed by player id, so the two sides of a
//! game never share a sequence counter.

use move_protocol::{MovePacket, MoveSequencer, ReorderOutcome};
use std::collections::HashMap;

/// What the session should do with an inbound move.
#[derive(Debug, PartialEq, Eq)]
pub enum MoveDecision {
    /// Release these moves to the lobby/Redis in the given order.
    Apply(Vec<String>),
    /// The move is early; hold it until the gap in front is filled.
    Buffered { buffered: usize },
    /// Already released — a duplicate or a late retransmission. Drop it.
    Duplicate,
    /// The reorder window elapsed with a gap still open. These sequence ids
    /// were never seen and should be asked for again.
    Retransmit(Vec<u64>),
}

pub struct MoveOrderGuard {
    sequencers: HashMap<String, MoveSequencer>,
}

impl Default for MoveOrderGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveOrderGuard {
    pub fn new() -> Self {
        Self {
            sequencers: HashMap::new(),
        }
    }

    fn sequencer_for(&mut self, player_id: &str) -> &mut MoveSequencer {
        self.sequencers
            .entry(player_id.to_string())
            .or_insert_with(MoveSequencer::new)
    }

    /// Number of moves currently held in the reorder buffer for `player_id`.
    pub fn buffered(&self, player_id: &str) -> usize {
        self.sequencers.get(player_id).map_or(0, MoveSequencer::pending_len)
    }

    /// The next sequence id the stream is waiting for.
    pub fn next_expected(&self, player_id: &str) -> u64 {
        self.sequencers
            .get(player_id)
            .map_or(1, MoveSequencer::next_expected_seq)
    }

    /// Advance the reorder clock for `player_id` without a new packet.
    ///
    /// A session calls this from its heartbeat: the reorder window is measured
    /// in server time, so something has to notice when it elapses even if the
    /// client goes quiet after sending a gapped move.
    pub fn tick(&mut self, player_id: &str, now_ms: u64) -> MoveDecision {
        let outcome = match self.sequencers.get_mut(player_id) {
            Some(sequencer) => sequencer.poll(now_ms),
            None => ReorderOutcome::default(),
        };
        self.classify(outcome)
    }

    /// Decide what to do with an inbound move.
    ///
    /// `seq` is `None` for clients that predate sequence stamping; those moves
    /// bypass the buffer entirely.
    pub fn accept(
        &mut self,
        player_id: &str,
        seq: Option<u64>,
        move_notation: &str,
        now_ms: u64,
    ) -> MoveDecision {
        let Some(seq) = seq else {
            return MoveDecision::Apply(vec![move_notation.to_string()]);
        };

        let packet = MovePacket::new(player_id, seq, move_notation, now_ms);
        let outcome = self.sequencer_for(player_id).submit(packet, now_ms);
        self.classify(outcome)
    }

    fn classify(&self, outcome: ReorderOutcome) -> MoveDecision {
        if !outcome.duplicates.is_empty() && outcome.ready.is_empty() && outcome.buffered == 0 {
            return MoveDecision::Duplicate;
        }
        if !outcome.ready.is_empty() {
            return MoveDecision::Apply(
                outcome
                    .ready
                    .iter()
                    .map(|packet| packet.move_notation.clone())
                    .collect(),
            );
        }
        if !outcome.retransmit.is_empty() {
            return MoveDecision::Retransmit(outcome.retransmit);
        }
        MoveDecision::Buffered {
            buffered: outcome.buffered,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsequenced_clients_bypass_the_buffer() {
        let mut guard = MoveOrderGuard::new();
        assert_eq!(
            guard.accept("white", None, "e4", 1_000),
            MoveDecision::Apply(vec!["e4".into()])
        );
        assert_eq!(guard.buffered("white"), 0);
        assert_eq!(guard.next_expected("white"), 1);
    }

    #[test]
    fn in_order_moves_are_applied_immediately() {
        let mut guard = MoveOrderGuard::new();
        assert_eq!(
            guard.accept("white", Some(1), "e4", 1_000),
            MoveDecision::Apply(vec!["e4".into()])
        );
        assert_eq!(
            guard.accept("white", Some(2), "e5", 1_010),
            MoveDecision::Apply(vec!["e5".into()])
        );
    }

    #[test]
    fn an_early_move_waits_for_the_gap_to_close() {
        let mut guard = MoveOrderGuard::new();
        assert_eq!(
            guard.accept("white", Some(2), "e5", 1_000),
            MoveDecision::Buffered { buffered: 1 }
        );
        assert_eq!(guard.buffered("white"), 1);
        assert_eq!(guard.next_expected("white"), 1);

        // The missing move arrives and both are released, in order.
        assert_eq!(
            guard.accept("white", Some(1), "e4", 1_010),
            MoveDecision::Apply(vec!["e4".into(), "e5".into()])
        );
        assert_eq!(guard.buffered("white"), 0);
    }

    #[test]
    fn duplicates_are_dropped() {
        let mut guard = MoveOrderGuard::new();
        guard.accept("white", Some(1), "e4", 1_000);
        assert_eq!(
            guard.accept("white", Some(1), "e4", 1_005),
            MoveDecision::Duplicate
        );
    }

    #[test]
    fn an_open_gap_asks_for_a_retransmit_once_the_window_elapses() {
        let mut guard = MoveOrderGuard::new();
        // seq 3 arrives first, so 1 and 2 are missing.
        assert!(matches!(
            guard.accept("white", Some(3), "Nf3", 1_000),
            MoveDecision::Buffered { buffered: 1 }
        ));

        // Still inside the 500ms reorder window: nothing is due yet.
        assert_eq!(guard.tick("white", 1_100), MoveDecision::Buffered { buffered: 1 });

        // Window elapsed with the gap still open.
        assert_eq!(
            guard.tick("white", 1_600),
            MoveDecision::Retransmit(vec![1, 2])
        );
    }

    #[test]
    fn ticking_an_unknown_player_is_a_no_op() {
        let mut guard = MoveOrderGuard::new();
        assert_eq!(
            guard.tick("white", 1_000),
            MoveDecision::Buffered { buffered: 0 }
        );
    }

    #[test]
    fn players_do_not_share_a_sequence_counter() {
        let mut guard = MoveOrderGuard::new();
        assert_eq!(
            guard.accept("white", Some(1), "e4", 1_000),
            MoveDecision::Apply(vec!["e4".into()])
        );
        // Black starts its own stream at 1 even though white already used 1.
        assert_eq!(
            guard.accept("black", Some(1), "e5", 1_000),
            MoveDecision::Apply(vec!["e5".into()])
        );
    }
}