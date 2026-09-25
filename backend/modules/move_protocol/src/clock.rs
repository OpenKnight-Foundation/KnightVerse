//! NTP-style clock-drift compensation.
//!
//! Mobile clients can sit hundreds of milliseconds away from server time, so a
//! move's `client_ts_ms` is not trustworthy on its own — the same move can
//! appear to be "late" purely because the client's clock is behind. Measuring
//! the offset once and correcting every timestamp removes that false signal.
//!
//! The server probes the client (`Ping`) and the client answers with its
//! receive/send times (`Pong`). With `t1` the server send time, `t2` the client
//! receive time, `t3` the client send time and `t4` the server receive time:
//!
//! ```text
//! round trip delay = (t4 - t1) - (t3 - t2)
//! clock offset     = ((t2 - t1) + (t3 - t4)) / 2
//! ```
//!
//! `clock offset` is defined as `client_clock - server_clock`. Because the
//! offset estimate is biased by `(d1 - d2) / 2` (the asymmetry between the two
//! one-way delays), the estimator keeps a sliding window of samples and uses
//! the one with the **lowest round-trip delay** — the NTP clock-filter
//! heuristic, which bounds the error by half the smallest observed RTT.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use crate::packet::{Ping, Pong};

/// How many recent samples the clock filter considers.
pub const DEFAULT_WINDOW: usize = 16;

/// Samples with a round-trip delay above this are discarded as implausible
/// (a stalled or wedged client would otherwise poison the estimate).
pub const DEFAULT_MAX_RTT_MS: u64 = 2_000;

/// One completed ping/pong round trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundTripSample {
    /// Estimated `client_clock - server_clock`, in milliseconds.
    pub offset_ms: i64,
    /// Measured round-trip delay, excluding client processing time.
    pub rtt_ms: u64,
}

/// Reasons a [`Pong`] could not be turned into a usable sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockSyncError {
    /// The pong does not match any outstanding ping (stale or forged reply).
    UnknownPing(u64),
    /// Timestamps went backwards, so the sample cannot be trusted.
    ClockWentBackwards,
    /// The round trip took longer than the configured maximum.
    ImplausibleRtt { rtt_ms: u64, max_rtt_ms: u64 },
}

impl fmt::Display for ClockSyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPing(id) => write!(f, "pong for unknown ping id {id}"),
            Self::ClockWentBackwards => write!(f, "timestamps are not monotonic"),
            Self::ImplausibleRtt { rtt_ms, max_rtt_ms } => write!(
                f,
                "round trip of {rtt_ms}ms exceeds the {max_rtt_ms}ms allowance"
            ),
        }
    }
}

impl std::error::Error for ClockSyncError {}

/// Tracks outstanding pings and the resulting drift samples for one client.
#[derive(Debug)]
pub struct ClockSync {
    window: usize,
    max_rtt_ms: u64,
    in_flight: BTreeMap<u64, u64>,
    samples: VecDeque<RoundTripSample>,
}

impl Default for ClockSync {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockSync {
    /// A tracker with the default window and RTT allowance.
    pub fn new() -> Self {
        Self::with_config(DEFAULT_WINDOW, DEFAULT_MAX_RTT_MS)
    }

    /// A tracker that keeps at most `window` samples.
    pub fn with_window(window: usize) -> Self {
        Self::with_config(window, DEFAULT_MAX_RTT_MS)
    }

    /// A tracker with a custom window and RTT allowance. `window` is clamped to
    /// at least 1 and `max_rtt_ms` to at least 1, so the estimator always has
    /// something to work with.
    pub fn with_config(window: usize, max_rtt_ms: u64) -> Self {
        Self {
            window: window.max(1),
            max_rtt_ms: max_rtt_ms.max(1),
            in_flight: BTreeMap::new(),
            samples: VecDeque::new(),
        }
    }

    /// Start a round trip. The returned ping must be sent to the client; the
    /// caller must remember `server_sent_ms` (it is embedded in the ping) and
    /// pass the receive time to [`Self::on_pong`].
    pub fn start_ping(&mut self, id: u64, server_sent_ms: u64) -> Ping {
        self.in_flight.insert(id, server_sent_ms);
        Ping { id, server_sent_ms }
    }

    /// Complete a round trip from a client [`Pong`] received at
    /// `server_recv_ms`, returning the sample or why it was rejected.
    pub fn on_pong(
        &mut self,
        pong: &Pong,
        server_recv_ms: u64,
    ) -> Result<RoundTripSample, ClockSyncError> {
        let t1 = self
            .in_flight
            .remove(&pong.id)
            .ok_or(ClockSyncError::UnknownPing(pong.id))?;
        let t2 = pong.client_recv_ms;
        let t3 = pong.client_send_ms;
        let t4 = server_recv_ms;

        if t4 < t1 || t3 < t2 {
            return Err(ClockSyncError::ClockWentBackwards);
        }

        let client_processing = t3 - t2;
        let rtt_ms = (t4 - t1).saturating_sub(client_processing);
        if rtt_ms > self.max_rtt_ms {
            return Err(ClockSyncError::ImplausibleRtt {
                rtt_ms,
                max_rtt_ms: self.max_rtt_ms,
            });
        }

        let offset_ms = ((t2 as i64 - t1 as i64) + (t3 as i64 - t4 as i64)) / 2;
        let sample = RoundTripSample { offset_ms, rtt_ms };

        self.samples.push_back(sample);
        while self.samples.len() > self.window {
            self.samples.pop_front();
        }

        Ok(sample)
    }

    /// The sample with the lowest round-trip delay in the window — the least
    /// biased estimate available. `None` until a sample has been collected.
    pub fn best_sample(&self) -> Option<RoundTripSample> {
        self.samples.iter().copied().min_by_key(|s| s.rtt_ms)
    }

    /// Estimated `client_clock - server_clock`, in milliseconds.
    pub fn offset_ms(&self) -> Option<i64> {
        self.best_sample().map(|s| s.offset_ms)
    }

    /// Round-trip delay of the currently selected sample, in milliseconds.
    pub fn rtt_ms(&self) -> Option<u64> {
        self.best_sample().map(|s| s.rtt_ms)
    }

    /// Translate a client timestamp into server time using the current offset.
    /// Returns `None` until at least one sample has been collected.
    pub fn client_to_server_ms(&self, client_ts_ms: i64) -> Option<i64> {
        self.offset_ms().map(|offset| client_ts_ms - offset)
    }

    /// The samples currently inside the window, oldest first.
    pub fn samples(&self) -> impl Iterator<Item = RoundTripSample> + '_ {
        self.samples.iter().copied()
    }

    /// Number of samples in the window.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Number of pings sent but not yet answered.
    pub fn in_flight(&self) -> usize {
        self.in_flight.len()
    }

    /// Forget all samples and outstanding pings.
    pub fn clear(&mut self) {
        self.in_flight.clear();
        self.samples.clear();
    }
}

#[cfg(test)]
mod test;
