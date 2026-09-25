//! Unit tests for NTP-style clock-drift compensation.

use super::*;

/// Small deterministic xorshift so the jitter model is reproducible.
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

    /// Uniform in `0..exclusive` (must be non-zero).
    fn below(&mut self, exclusive: u64) -> u64 {
        self.next_u64() % exclusive
    }
}

/// Feed one symmetric round trip through the estimator and return the sample.
fn probe(
    clock: &mut ClockSync,
    id: u64,
    server_sent: u64,
    one_way_ms: u64,
    offset: i64,
) -> RoundTripSample {
    let ping = clock.start_ping(id, server_sent);
    let client_recv = (server_sent as i64 + one_way_ms as i64 + offset) as u64;
    let client_send = client_recv;
    let server_recv = server_sent + one_way_ms * 2;
    clock
        .on_pong(&Pong::new(ping.id, client_recv, client_send), server_recv)
        .expect("symmetric probe should be accepted")
}

#[test]
fn test_symmetric_round_trip_recovers_offset_exactly() {
    let mut clock = ClockSync::new();

    let sample = probe(&mut clock, 1, 10_000, 20, 300);

    assert_eq!(sample.offset_ms, 300);
    assert_eq!(sample.rtt_ms, 40);
    assert_eq!(clock.offset_ms(), Some(300));
    assert_eq!(clock.rtt_ms(), Some(40));
}

#[test]
fn test_client_to_server_time_applies_the_offset() {
    let mut clock = ClockSync::new();
    probe(&mut clock, 1, 10_000, 20, 300);

    // Client clock is 300ms ahead, so its timestamps must be pulled back.
    assert_eq!(clock.client_to_server_ms(10_320), Some(10_020));
    assert_eq!(clock.client_to_server_ms(11_000), Some(10_700));
}

#[test]
fn test_client_to_server_time_is_none_before_any_sample() {
    let clock = ClockSync::new();
    assert_eq!(clock.offset_ms(), None);
    assert_eq!(clock.rtt_ms(), None);
    assert_eq!(clock.client_to_server_ms(10_000), None);
}

#[test]
fn test_offset_accuracy_stays_within_15ms_under_jitter() {
    // A client whose clock is 420ms *behind* server time, on a link whose
    // one-way delay swings between 5ms and 60ms in each direction.
    const TRUE_OFFSET_MS: i64 = -420;
    const WINDOW: usize = 64;
    const PROBES: u64 = 60;

    let mut clock = ClockSync::with_config(WINDOW, 5_000);
    let mut rng = Rng::new(0x5EED_1234_5678_9ABC);
    let mut server_sent = 10_000_000u64;

    for id in 0..PROBES {
        let outbound = 5 + rng.below(56); // 5..=60ms
        let inbound = 5 + rng.below(56); // 5..=60ms
        let client_processing = rng.below(4); // 0..=3ms

        let ping = clock.start_ping(id, server_sent);
        let client_recv = (server_sent as i64 + outbound as i64 + TRUE_OFFSET_MS) as u64;
        let client_send = client_recv + client_processing;
        let server_recv = server_sent + outbound + client_processing + inbound;

        clock
            .on_pong(&Pong::new(ping.id, client_recv, client_send), server_recv)
            .expect("a probe within the RTT allowance is accepted");

        server_sent += 1_000;
    }

    let estimate = clock.offset_ms().expect("samples were collected");
    let error = (estimate - TRUE_OFFSET_MS).abs();

    assert!(
        error <= 15,
        "clock drift estimate {estimate}ms is {error}ms away from the true offset {TRUE_OFFSET_MS}ms"
    );

    // The estimator must be using the least-delayed sample, not an average.
    assert_eq!(clock.sample_count(), PROBES as usize);
    let best = clock.best_sample().unwrap();
    let min_rtt = clock.samples().map(|s| s.rtt_ms).min().unwrap();
    let mean_rtt = clock.samples().map(|s| s.rtt_ms).sum::<u64>() / PROBES;
    assert_eq!(best.rtt_ms, min_rtt);
    assert!(best.rtt_ms < mean_rtt);
}

#[test]
fn test_sample_selection_prefers_the_lowest_round_trip() {
    let mut clock = ClockSync::new();

    probe(&mut clock, 1, 1_000, 45, 100); // rtt 90
    probe(&mut clock, 2, 2_000, 10, 250); // rtt 20 <- best
    probe(&mut clock, 3, 3_000, 30, 130); // rtt 60

    assert_eq!(clock.rtt_ms(), Some(20));
    assert_eq!(clock.offset_ms(), Some(250));
    assert_eq!(clock.sample_count(), 3);
}

#[test]
fn test_window_bounds_the_sample_history() {
    let mut clock = ClockSync::with_window(2);

    probe(&mut clock, 1, 1_000, 10, 5);
    probe(&mut clock, 2, 2_000, 20, 6);
    probe(&mut clock, 3, 3_000, 30, 7);

    assert_eq!(clock.sample_count(), 2, "only the newest samples are kept");
    assert_eq!(
        clock.offset_ms(),
        Some(6),
        "sample 1 fell out of the window"
    );
}

#[test]
fn test_window_of_zero_is_clamped_to_one() {
    let mut clock = ClockSync::with_config(0, DEFAULT_MAX_RTT_MS);
    probe(&mut clock, 1, 1_000, 10, 42);
    assert_eq!(clock.sample_count(), 1);
    assert_eq!(clock.offset_ms(), Some(42));
}

#[test]
fn test_unknown_ping_is_rejected() {
    let mut clock = ClockSync::new();

    let err = clock
        .on_pong(&Pong::new(99, 1_000, 1_000), 1_020)
        .unwrap_err();

    assert_eq!(err, ClockSyncError::UnknownPing(99));
    assert_eq!(clock.sample_count(), 0);
}

#[test]
fn test_backwards_timestamps_are_rejected() {
    let mut clock = ClockSync::new();

    // Server receive time before the ping was even sent.
    clock.start_ping(1, 1_000);
    let err = clock.on_pong(&Pong::new(1, 1_000, 1_000), 900).unwrap_err();
    assert_eq!(err, ClockSyncError::ClockWentBackwards);

    // Client send time before it received the ping.
    clock.start_ping(2, 1_000);
    let err = clock
        .on_pong(&Pong::new(2, 1_050, 1_040), 1_100)
        .unwrap_err();
    assert_eq!(err, ClockSyncError::ClockWentBackwards);

    assert_eq!(clock.sample_count(), 0);
}

#[test]
fn test_implausible_round_trip_is_discarded() {
    let mut clock = ClockSync::with_config(DEFAULT_WINDOW, 100);

    clock.start_ping(1, 0);
    let err = clock
        .on_pong(&Pong::new(1, 1_000, 1_000), 5_000)
        .unwrap_err();

    assert_eq!(
        err,
        ClockSyncError::ImplausibleRtt {
            rtt_ms: 5_000,
            max_rtt_ms: 100
        }
    );
    assert_eq!(clock.sample_count(), 0);
    assert_eq!(clock.offset_ms(), None);
}

#[test]
fn test_outstanding_pings_are_tracked_until_answered() {
    let mut clock = ClockSync::new();

    clock.start_ping(1, 1_000);
    clock.start_ping(2, 2_000);
    assert_eq!(clock.in_flight(), 2);

    // Answer the second probe first — correlation is by id, not by order.
    let ping = clock.start_ping(3, 3_000);
    assert_eq!(clock.in_flight(), 3);
    clock.on_pong(&Pong::new(2, 2_010, 2_010), 2_020).unwrap();
    assert_eq!(clock.in_flight(), 2);
    assert_eq!(ping.id, 3);
}

#[test]
fn test_clear_forgets_samples_and_pings() {
    let mut clock = ClockSync::new();
    probe(&mut clock, 1, 1_000, 10, 42);
    clock.start_ping(2, 2_000);

    clock.clear();

    assert_eq!(clock.sample_count(), 0);
    assert_eq!(clock.in_flight(), 0);
    assert_eq!(clock.offset_ms(), None);
}
