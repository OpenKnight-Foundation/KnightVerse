//! Unit tests for sequence assignment and out-of-order move resolution.

use super::*;
use crate::MovePacket;

/// Small deterministic xorshift so jitter/reordering tests are reproducible.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `0..n` (n must be non-zero).
    fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }
}

fn move_packet(seq: u64, notation: &str) -> MovePacket {
    MovePacket::new("white", seq, notation, 1_000 + seq)
}

#[test]
fn test_seq_assigner_is_strictly_increasing() {
    let mut assigner = SeqAssigner::new();
    assert_eq!(assigner.peek(), 1);

    let ids: Vec<u64> = (0..5).map(|_| assigner.next_seq()).collect();
    assert_eq!(ids, vec![1, 2, 3, 4, 5]);

    for pair in ids.windows(2) {
        assert!(pair[0] < pair[1], "sequence ids must strictly increase");
    }
    assert_eq!(assigner.peek(), 6);
}

#[test]
fn test_in_order_moves_release_immediately() {
    let mut sequencer = MoveSequencer::new();

    let outcome = sequencer.submit(move_packet(1, "e2e4"), 0);

    assert_eq!(outcome.ready.len(), 1);
    assert_eq!(outcome.ready[0].move_notation, "e2e4");
    assert!(outcome.retransmit.is_empty());
    assert!(outcome.duplicates.is_empty());
    assert_eq!(sequencer.next_expected_seq(), 2);
    assert_eq!(sequencer.pending_len(), 0);
}

#[test]
fn test_out_of_order_moves_are_reassembled_in_sequence_order() {
    let mut sequencer = MoveSequencer::new();

    // Moves 3 and 4 overtake 1 and 2 on the wire.
    let outcome = sequencer.submit(move_packet(3, "Ng1f3"), 0);
    assert!(outcome.ready.is_empty());
    assert_eq!(outcome.buffered, 1);

    sequencer.submit(move_packet(4, "Bf1c4"), 2);

    let outcome = sequencer.submit(move_packet(1, "e2e4"), 4);
    assert_eq!(outcome.ready.len(), 1, "only move 1 is contiguous yet");

    // Move 2 finally arrives, unlocking everything behind it.
    let outcome = sequencer.submit(move_packet(2, "e7e5"), 6);

    let notations: Vec<&str> = outcome
        .ready
        .iter()
        .map(|m| m.move_notation.as_str())
        .collect();
    assert_eq!(notations, vec!["e7e5", "Ng1f3", "Bf1c4"]);
    assert_eq!(outcome.buffered, 0);
    assert_eq!(sequencer.next_expected_seq(), 5);

    // No valid move was rejected anywhere along the way.
    assert!(outcome.duplicates.is_empty());
    assert!(outcome.retransmit.is_empty());
}

#[test]
fn test_no_retransmission_before_the_buffer_window() {
    let mut sequencer = MoveSequencer::new();

    sequencer.submit(move_packet(1, "e2e4"), 0);
    let outcome = sequencer.submit(move_packet(4, "Bf1c4"), 0);

    assert!(outcome.retransmit.is_empty());
    assert_eq!(outcome.buffered, 1);

    // Still inside the 500ms window.
    let outcome = sequencer.poll(499);
    assert!(outcome.retransmit.is_empty());
}

#[test]
fn test_retransmission_requested_once_the_window_elapses() {
    let mut sequencer = MoveSequencer::new();
    assert_eq!(sequencer.buffer_ms(), DEFAULT_REORDER_BUFFER_MS);

    sequencer.submit(move_packet(1, "e2e4"), 0);
    sequencer.submit(move_packet(4, "Bf1c4"), 0);

    let outcome = sequencer.poll(DEFAULT_REORDER_BUFFER_MS);
    assert_eq!(outcome.retransmit, vec![2, 3]);
    // Nothing was discarded while we wait for the resend.
    assert_eq!(outcome.buffered, 1);
    assert!(outcome.ready.is_empty());
}

#[test]
fn test_still_missing_moves_are_requested_again_later() {
    let mut sequencer = MoveSequencer::new();

    sequencer.submit(move_packet(1, "e2e4"), 0);
    sequencer.submit(move_packet(3, "Ng1f3"), 0);

    assert_eq!(sequencer.poll(500).retransmit, vec![2]);
    assert!(sequencer.poll(700).retransmit.is_empty());
    assert_eq!(sequencer.poll(1_000).retransmit, vec![2]);
}

#[test]
fn test_retransmitted_move_unblocks_the_buffer_in_order() {
    let mut sequencer = MoveSequencer::new();

    sequencer.submit(move_packet(1, "e2e4"), 0);
    sequencer.submit(move_packet(3, "Ng1f3"), 0);
    assert_eq!(sequencer.poll(500).retransmit, vec![2]);

    // The client resends move 2 in answer to the request.
    let outcome = sequencer.submit(move_packet(2, "e7e5"), 520);

    let seqs: Vec<u64> = outcome.ready.iter().map(|m| m.seq).collect();
    assert_eq!(seqs, vec![2, 3]);
    assert_eq!(sequencer.next_expected_seq(), 4);
}

#[test]
fn test_duplicate_and_stale_packets_are_ignored_not_rejected() {
    let mut sequencer = MoveSequencer::new();

    sequencer.submit(move_packet(1, "e2e4"), 0);

    // An already-released move arriving again.
    let outcome = sequencer.submit(move_packet(1, "e2e4"), 10);
    assert_eq!(outcome.duplicates, vec![1]);
    assert!(outcome.ready.is_empty());

    // A move buffered twice.
    sequencer.submit(move_packet(3, "Ng1f3"), 10);
    let outcome = sequencer.submit(move_packet(3, "Ng1f3"), 20);
    assert_eq!(outcome.duplicates, vec![3]);
    assert_eq!(outcome.buffered, 1);
}

#[test]
fn test_poll_without_a_gap_is_a_noop() {
    let mut sequencer = MoveSequencer::new();
    sequencer.submit(move_packet(1, "e2e4"), 0);

    let outcome = sequencer.poll(60_000);
    assert!(outcome.is_empty());
    assert_eq!(outcome.buffered, 0);
}

#[test]
fn test_jittered_and_reordered_stream_is_never_reordered() {
    const MOVES: u64 = 64;

    let mut sequencer = MoveSequencer::new();
    let mut rng = Rng::new(0xC0FF_EE00_1234_5678);

    // Shuffle the arrival order, then hand each packet a small random delay.
    let mut order: Vec<u64> = (1..=MOVES).collect();
    for i in (1..order.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        order.swap(i, j);
    }

    let mut released = Vec::new();
    let mut now_ms = 0u64;

    for seq in order {
        now_ms += rng.below(7); // 0..=6ms of jitter between arrivals
                                // Every second, ask the sequencer for anything that is now overdue.
        released.extend(sequencer.poll(now_ms).ready);
        released.extend(sequencer.submit(move_packet(seq, "e2e4"), now_ms).ready);
    }
    // Drain anything that was only waiting on a gap deadline that never fired.
    released.extend(sequencer.poll(now_ms + 10_000).ready);

    let seqs: Vec<u64> = released.iter().map(|m| m.seq).collect();
    assert_eq!(seqs.len(), MOVES as usize, "no move may be lost");
    assert_eq!(
        seqs,
        (1..=MOVES).collect::<Vec<u64>>(),
        "moves must be released in sequence order"
    );
    assert_eq!(sequencer.pending_len(), 0);
}

#[test]
fn test_capacity_overflow_flushes_without_dropping_moves() {
    // Room for 4 early moves, but move 1 never arrives.
    let mut sequencer = MoveSequencer::with_config(DEFAULT_REORDER_BUFFER_MS, 4);

    let mut ready = Vec::new();
    for seq in 2..=6 {
        ready.extend(sequencer.submit(move_packet(seq, "e2e4"), 0).ready);
    }

    let seqs: Vec<u64> = ready.iter().map(|m| m.seq).collect();
    assert_eq!(seqs, vec![2, 3, 4, 5, 6], "every move is still released");
    assert_eq!(sequencer.pending_len(), 0);
    assert_eq!(sequencer.next_expected_seq(), 7);
}

#[test]
fn test_reset_clears_all_state() {
    let mut sequencer = MoveSequencer::new();
    sequencer.submit(move_packet(1, "e2e4"), 0);
    sequencer.submit(move_packet(4, "Bf1c4"), 0);

    sequencer.reset();

    assert_eq!(sequencer.next_expected_seq(), 1);
    assert_eq!(sequencer.pending_len(), 0);
    // A fresh move 1 is accepted again after the reset.
    let outcome = sequencer.submit(move_packet(1, "d2d4"), 0);
    assert_eq!(outcome.ready.len(), 1);
}
