//! Sequence-id assignment (client side) and out-of-order move resolution
//! (server side).
//!
//! The server keeps a reorder buffer keyed by sequence id. Packets that arrive
//! in order are released immediately; packets that arrive early are buffered
//! and released as soon as the gap in front of them is filled. If a gap is
//! still open after [`MoveSequencer::buffer_ms`], the sequencer asks for the
//! missing packet to be resent rather than stalling forever.

use std::collections::BTreeMap;

use crate::packet::MovePacket;

/// Default reorder window: how long the server waits for a missing move
/// before requesting retransmission.
pub const DEFAULT_REORDER_BUFFER_MS: u64 = 500;

/// Default cap on how many packets may sit in the reorder buffer at once, so a
/// client that simply stops sending one move cannot grow server memory without
/// bound.
pub const DEFAULT_MAX_BUFFERED: usize = 256;

/// Client-side generator of strictly monotonically increasing sequence ids.
#[derive(Debug, Clone)]
pub struct SeqAssigner {
    next: u64,
}

impl Default for SeqAssigner {
    fn default() -> Self {
        Self::new()
    }
}

impl SeqAssigner {
    /// Sequence ids start at `1`, so `0` can never be confused with a real id.
    pub fn new() -> Self {
        Self { next: 1 }
    }

    /// Returns the next id and advances the counter. Values never repeat and
    /// never decrease.
    pub fn next_seq(&mut self) -> u64 {
        let seq = self.next;
        self.next += 1;
        seq
    }

    /// The id that [`Self::next_seq`] will return next.
    pub fn peek(&self) -> u64 {
        self.next
    }
}

/// Everything the caller needs to do after handing a packet to the sequencer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReorderOutcome {
    /// Moves released in strictly increasing sequence order, ready to apply.
    pub ready: Vec<MovePacket>,
    /// Sequence ids the server should ask the client to resend.
    pub retransmit: Vec<u64>,
    /// Ids that were already seen (or already released) and safely ignored.
    pub duplicates: Vec<u64>,
    /// How many packets are still held in the reorder buffer.
    pub buffered: usize,
    /// Set when the buffer hit its capacity and was flushed early. The moves
    /// are all released — none are dropped — but the gap in front of them was
    /// abandoned.
    pub force_flushed: bool,
}

impl ReorderOutcome {
    /// True when there is nothing for the caller to act on.
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty() && self.retransmit.is_empty() && self.duplicates.is_empty()
    }
}

/// Server-side reorder buffer for one move stream (one player, one game).
#[derive(Debug)]
pub struct MoveSequencer {
    buffer_ms: u64,
    capacity: usize,
    next_seq: u64,
    pending: BTreeMap<u64, MovePacket>,
    /// Missing seq -> the time at which we should (re)request it.
    gap_deadlines: BTreeMap<u64, u64>,
}

impl Default for MoveSequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl MoveSequencer {
    /// A sequencer with the default 500ms window and 256-packet buffer.
    pub fn new() -> Self {
        Self::with_config(DEFAULT_REORDER_BUFFER_MS, DEFAULT_MAX_BUFFERED)
    }

    /// A sequencer with a custom retransmission window and buffer capacity.
    pub fn with_config(buffer_ms: u64, capacity: usize) -> Self {
        Self {
            buffer_ms,
            capacity,
            next_seq: 1,
            pending: BTreeMap::new(),
            gap_deadlines: BTreeMap::new(),
        }
    }

    /// The retransmission window, in milliseconds.
    pub fn buffer_ms(&self) -> u64 {
        self.buffer_ms
    }

    /// The reorder buffer's capacity, in packets.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The next sequence id the stream is waiting for.
    pub fn next_expected_seq(&self) -> u64 {
        self.next_seq
    }

    /// How many packets are currently held out of order.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Submit a move packet that arrived at server time `now_ms`.
    pub fn submit(&mut self, packet: MovePacket, now_ms: u64) -> ReorderOutcome {
        let mut outcome = ReorderOutcome::default();
        let seq = packet.seq;

        if seq < self.next_seq {
            // Already released: a retransmission we no longer need.
            outcome.duplicates.push(seq);
            return self.finish(outcome, now_ms);
        }

        if seq == self.next_seq {
            self.release(&mut outcome, packet);
            self.drain_contiguous(&mut outcome);
            return self.finish(outcome, now_ms);
        }

        // seq > next_seq: the packet is early, so a gap exists in front of it.
        if self.pending.contains_key(&seq) {
            outcome.duplicates.push(seq);
            return self.finish(outcome, now_ms);
        }

        for missing in self.next_seq..seq {
            if !self.pending.contains_key(&missing) {
                // Keep the earliest deadline so a gap's 500ms window is
                // measured from when it was first observed.
                self.gap_deadlines
                    .entry(missing)
                    .or_insert(now_ms + self.buffer_ms);
            }
        }

        self.pending.insert(seq, packet);

        if self.pending.len() > self.capacity {
            self.force_flush(&mut outcome);
        }

        self.finish(outcome, now_ms)
    }

    /// Advance time without submitting a packet — call this from a timer so
    /// retransmission requests still fire when a client goes quiet.
    pub fn poll(&mut self, now_ms: u64) -> ReorderOutcome {
        let outcome = ReorderOutcome::default();
        self.finish(outcome, now_ms)
    }

    /// Drop all state, e.g. when a game ends.
    pub fn reset(&mut self) {
        self.next_seq = 1;
        self.pending.clear();
        self.gap_deadlines.clear();
    }

    /// Release an in-order packet and advance the expected id.
    fn release(&mut self, outcome: &mut ReorderOutcome, packet: MovePacket) {
        self.gap_deadlines.remove(&packet.seq);
        self.next_seq = packet.seq + 1;
        outcome.ready.push(packet);
    }

    /// Release every buffered packet that is now contiguous with `next_seq`.
    fn drain_contiguous(&mut self, outcome: &mut ReorderOutcome) {
        while let Some(packet) = self.pending.remove(&self.next_seq) {
            self.release(outcome, packet);
        }
    }

    /// Release the buffer in sequence order after hitting capacity. No packet
    /// is dropped; the unresolved gap is abandoned instead.
    fn force_flush(&mut self, outcome: &mut ReorderOutcome) {
        outcome.force_flushed = true;
        let held = std::mem::take(&mut self.pending);
        for (_, packet) in held {
            self.release(outcome, packet);
        }
        // Anything still outstanding was skipped, so it is no longer expected.
        self.gap_deadlines.clear();
    }

    /// Collect retransmission requests that are due and record buffer depth.
    fn finish(&mut self, mut outcome: ReorderOutcome, now_ms: u64) -> ReorderOutcome {
        let due: Vec<u64> = self
            .gap_deadlines
            .iter()
            .filter(|(_, deadline)| **deadline <= now_ms)
            .map(|(seq, _)| *seq)
            .collect();

        for seq in due {
            // Re-arm so a still-missing move is requested again later rather
            // than silently waiting forever.
            self.gap_deadlines.insert(seq, now_ms + self.buffer_ms);
            if !outcome.retransmit.contains(&seq) {
                outcome.retransmit.push(seq);
            }
        }
        outcome.retransmit.sort_unstable();

        outcome.buffered = self.pending.len();
        outcome
    }
}

#[cfg(test)]
mod test;
