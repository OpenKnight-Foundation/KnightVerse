//! BE-77 — out-of-order move resolution and timestamp drift compensation.
//!
//! On high-latency mobile networks two problems corrupt move ordering:
//!
//! 1. **Reordering.** Packets take different paths, so a move sent after
//!    another can arrive first. Arrival order is therefore not move order.
//! 2. **Clock drift.** A client's wall clock can be hundreds of milliseconds
//!    away from server time, so a timestamp is not comparable across peers.
//!
//! This crate solves both without rejecting valid moves:
//!
//! * Every client move carries a strictly increasing sequence id from a
//!   [`SeqAssigner`]. The server feeds packets to a [`MoveSequencer`], which
//!   releases them in sequence order, buffers early arrivals for up to 500ms,
//!   and asks for missing packets to be resent instead of dropping the moves
//!   queued behind them.
//! * A [`ClockSync`] runs an NTP-style ping/pong exchange and reports the
//!   client's clock offset, so client timestamps can be corrected into server
//!   time before they are used for anything time-sensitive.
//!
//! # Example
//!
//! ```
//! use move_protocol::{ClockSync, MovePacket, MoveSequencer, Pong, SeqAssigner};
//!
//! // Client: stamp each move with the next sequence id.
//! let mut assigner = SeqAssigner::new();
//! let second = MovePacket::new("white", assigner.next_seq(), "e2e4", 10_000);
//! assert_eq!(second.seq, 1);
//!
//! // Server: reassemble a stream that arrives out of order.
//! let mut sequencer = MoveSequencer::new();
//! let early = MovePacket::new("white", 3, "Ng1f3", 10_100);
//! let outcome = sequencer.submit(early, 0);
//! assert!(outcome.ready.is_empty());
//! assert_eq!(sequencer.next_expected_seq(), 1);
//!
//! let first = MovePacket::new("white", 1, "e2e4", 10_000);
//! let outcome = sequencer.submit(first, 5);
//! assert_eq!(outcome.ready.len(), 1);
//! assert_eq!(outcome.ready[0].move_notation, "e2e4");
//!
//! // Server: learn the client's clock offset with a ping/pong round trip.
//! let mut clock = ClockSync::new();
//! let ping = clock.start_ping(1, 10_000);
//! let pong = Pong::new(ping.id, 10_320, 10_320);
//! let sample = clock.on_pong(&pong, 10_040).unwrap();
//! assert_eq!(sample.offset_ms, 300);
//! assert_eq!(clock.client_to_server_ms(10_320), Some(10_020));
//! ```

#![forbid(unsafe_code)]

mod clock;
mod packet;
mod sequencer;

pub use clock::{ClockSync, ClockSyncError, RoundTripSample, DEFAULT_MAX_RTT_MS, DEFAULT_WINDOW};
pub use packet::{MovePacket, Ping, Pong};
pub use sequencer::{
    MoveSequencer, ReorderOutcome, SeqAssigner, DEFAULT_MAX_BUFFERED, DEFAULT_REORDER_BUFFER_MS,
};
