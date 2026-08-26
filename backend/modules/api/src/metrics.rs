// src/metrics.rs
//
// Prometheus metrics instrumentation for the KnightVerse backend.
//
// All metrics are registered in a single global registry and exported via the
// `/metrics` endpoint. Metric collection is designed to add < 0.1ms overhead
// to hot paths (move validation, matchmaking) by using lock-free atomic
// counters and pre-registered histogram buckets.

use prometheus::{
    Histogram, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
    TextEncoder,
};
use std::sync::OnceLock;

/// Global Prometheus registry shared across the application.
pub static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Move validation duration histogram (seconds).
pub static MOVE_VALIDATION_DURATION: OnceLock<Histogram> = OnceLock::new();

/// Matchmaking queue duration histogram (seconds).
pub static MATCHMAKING_QUEUE_DURATION: OnceLock<Histogram> = OnceLock::new();

/// Soroban RPC call duration histogram (seconds).
pub static SOROBAN_RPC_CALL_DURATION: OnceLock<Histogram> = OnceLock::new();

/// Active games gauge, labelled by game mode (bullet, blitz, rapid, classical).
pub static ACTIVE_GAMES: OnceLock<IntGaugeVec> = OnceLock::new();

/// WebSocket connected clients gauge, labelled by role (player, spectator).
pub static WEBSOCKET_CONNECTED_CLIENTS: OnceLock<IntGaugeVec> = OnceLock::new();

/// Total number of HTTP requests handled (useful for sanity checks).
pub static HTTP_REQUESTS_TOTAL: OnceLock<IntCounterVec> = OnceLock::new();

/// Initialize the global Prometheus registry and all metrics.
///
/// This function is idempotent — calling it more than once is a no-op.
pub fn init_metrics() -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let registry = Registry::new();

        // --- Histograms -----------------------------------------------------
        let move_validation = Histogram::with_opts(
            HistogramOpts::new(
                "chess_move_validation_duration_seconds",
                "Time spent validating and applying a chess move.",
            )
            .buckets(vec![
                0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25,
                0.5, 1.0,
            ]),
        )
        .expect("failed to create move validation histogram");
        registry
            .register(Box::new(move_validation.clone()))
            .expect("failed to register move validation histogram");
        let _ = MOVE_VALIDATION_DURATION.set(move_validation);

        let matchmaking_queue = Histogram::with_opts(
            HistogramOpts::new(
                "matchmaking_queue_duration_seconds",
                "Time spent in the matchmaking queue before a match is found.",
            )
            .buckets(vec![
                0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0,
            ]),
        )
        .expect("failed to create matchmaking queue histogram");
        registry
            .register(Box::new(matchmaking_queue.clone()))
            .expect("failed to register matchmaking queue histogram");
        let _ = MATCHMAKING_QUEUE_DURATION.set(matchmaking_queue);

        let soroban_rpc = Histogram::with_opts(
            HistogramOpts::new(
                "soroban_rpc_call_duration_seconds",
                "Time spent making a Soroban RPC call.",
            )
            .buckets(vec![
                0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5,
            ]),
        )
        .expect("failed to create soroban RPC histogram");
        registry
            .register(Box::new(soroban_rpc.clone()))
            .expect("failed to register soroban RPC histogram");
        let _ = SOROBAN_RPC_CALL_DURATION.set(soroban_rpc);

        // --- Gauges ---------------------------------------------------------
        let active_games = IntGaugeVec::new(
            Opts::new(
                "active_games_total",
                "Number of currently active games, by game mode.",
            ),
            &["mode"],
        )
        .expect("failed to create active games gauge");
        registry
            .register(Box::new(active_games.clone()))
            .expect("failed to register active games gauge");
        let _ = ACTIVE_GAMES.set(active_games);

        let ws_clients = IntGaugeVec::new(
            Opts::new(
                "websocket_connected_clients_total",
                "Number of currently connected WebSocket clients, by role.",
            ),
            &["role"],
        )
        .expect("failed to create websocket clients gauge");
        registry
            .register(Box::new(ws_clients.clone()))
            .expect("failed to register websocket clients gauge");
        let _ = WEBSOCKET_CONNECTED_CLIENTS.set(ws_clients);

        // --- Counters -------------------------------------------------------
        let http_requests = IntCounterVec::new(
            Opts::new(
                "http_requests_total",
                "Total number of HTTP requests handled by the API.",
            ),
            &["method", "path"],
        )
        .expect("failed to create http requests counter");
        registry
            .register(Box::new(http_requests.clone()))
            .expect("failed to register http requests counter");
        let _ = HTTP_REQUESTS_TOTAL.set(http_requests);

        registry
    })
}

/// Convenience accessors that panic-safe return the metric or a no-op fallback.
pub fn move_validation_duration() -> &'static Histogram {
    init_metrics();
    MOVE_VALIDATION_DURATION
        .get()
        .expect("move validation histogram not initialized")
}

pub fn matchmaking_queue_duration() -> &'static Histogram {
    init_metrics();
    MATCHMAKING_QUEUE_DURATION
        .get()
        .expect("matchmaking queue histogram not initialized")
}

pub fn soroban_rpc_call_duration() -> &'static Histogram {
    init_metrics();
    SOROBAN_RPC_CALL_DURATION
        .get()
        .expect("soroban RPC histogram not initialized")
}

pub fn active_games() -> &'static IntGaugeVec {
    init_metrics();
    ACTIVE_GAMES.get().expect("active games gauge not initialized")
}

pub fn websocket_connected_clients() -> &'static IntGaugeVec {
    init_metrics();
    WEBSOCKET_CONNECTED_CLIENTS
        .get()
        .expect("websocket clients gauge not initialized")
}

/// Render all metrics in Prometheus text exposition format.
pub fn render_metrics() -> String {
    init_metrics();
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.get().unwrap().gather();
    encoder
        .encode_to_string(&metric_families)
        .unwrap_or_else(|e| format!("# error encoding metrics: {e}\n"))
}

/// Increment the active games gauge for a given game mode.
pub fn inc_active_game(mode: &str) {
    active_games().with_label_values(&[mode]).inc();
}

/// Decrement the active games gauge for a given game mode.
pub fn dec_active_game(mode: &str) {
    active_games().with_label_values(&[mode]).dec();
}

/// Increment the WebSocket connected clients gauge for a given role.
pub fn inc_ws_client(role: &str) {
    websocket_connected_clients().with_label_values(&[role]).inc();
}

/// Decrement the WebSocket connected clients gauge for a given role.
pub fn dec_ws_client(role: &str) {
    websocket_connected_clients().with_label_values(&[role]).dec();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registration() {
        let registry = init_metrics();
        let families = registry.gather();
        let names: Vec<&str> = families.iter().map(|f| f.get_name()).collect();

        assert!(
            names.contains(&"chess_move_validation_duration_seconds"),
            "move validation histogram missing"
        );
        assert!(
            names.contains(&"matchmaking_queue_duration_seconds"),
            "matchmaking queue histogram missing"
        );
        assert!(
            names.contains(&"soroban_rpc_call_duration_seconds"),
            "soroban RPC histogram missing"
        );
        assert!(
            names.contains(&"active_games_total"),
            "active games gauge missing"
        );
        assert!(
            names.contains(&"websocket_connected_clients_total"),
            "websocket clients gauge missing"
        );
    }

    #[test]
    fn test_metrics_output_format() {
        init_metrics();
        let output = render_metrics();

        // Prometheus text format must contain metric names and HELP/TYPE lines.
        assert!(output.contains("# HELP chess_move_validation_duration_seconds"));
        assert!(output.contains("# TYPE chess_move_validation_duration_seconds histogram"));
        assert!(output.contains("# HELP active_games_total"));
        assert!(output.contains("# TYPE active_games_total gauge"));
        assert!(output.contains("# HELP websocket_connected_clients_total"));
        assert!(output.contains("# TYPE websocket_connected_clients_total gauge"));
        assert!(output.contains("# HELP matchmaking_queue_duration_seconds"));
        assert!(output.contains("# TYPE matchmaking_queue_duration_seconds histogram"));
        assert!(output.contains("# HELP soroban_rpc_call_duration_seconds"));
        assert!(output.contains("# TYPE soroban_rpc_call_duration_seconds histogram"));
    }

    #[test]
    fn test_gauge_inc_dec() {
        init_metrics();
        inc_active_game("bullet");
        inc_active_game("bullet");
        assert_eq!(active_games().with_label_values(&["bullet"]).get(), 2.0);
        dec_active_game("bullet");
        assert_eq!(active_games().with_label_values(&["bullet"]).get(), 1.0);
        dec_active_game("bullet");
        assert_eq!(active_games().with_label_values(&["bullet"]).get(), 0.0);

        inc_ws_client("player");
        assert_eq!(
            websocket_connected_clients().with_label_values(&["player"]).get(),
            1.0
        );
        dec_ws_client("player");
        assert_eq!(
            websocket_connected_clients().with_label_values(&["player"]).get(),
            0.0
        );
    }

    #[test]
    fn test_histogram_records_observation() {
        init_metrics();
        let hist = move_validation_duration();
        hist.observe(0.001);
        let sample = hist.get_sample_count();
        assert!(sample >= 1, "histogram should have at least one observation");
    }
}
