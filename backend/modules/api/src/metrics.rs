use prometheus::{Registry, CounterVec, Gauge, Histogram, HistogramOpts, Opts, Encoder, TextEncoder};
use std::sync::Arc;
use once_cell::sync::{Lazy, OnceCell};
use actix_web::{HttpResponse, web};

/// Global metrics registry
static REGISTRY: Lazy<Registry> = Lazy::new(|| Registry::new());

/// Global metrics instance
static GLOBAL_METRICS: OnceCell<Arc<Metrics>> = OnceCell::new();

/// Track if metrics have been registered to prevent duplicate registrations
static METRICS_REGISTERED: OnceCell<bool> = OnceCell::new();

/// Custom metrics for XLMate backend
pub struct Metrics {
    /// Number of currently active games
    pub active_games: Gauge,
    
    /// Number of active WebSocket connections
    pub ws_connections: Gauge,
    
    /// Database query duration in seconds
    pub db_query_duration: Histogram,
    
    /// Number of players in matchmaking queue
    pub matchmaking_queue_size: Gauge,
    
    /// Total AI requests (labeled by type: suggestion, analysis)
    pub ai_requests_total: CounterVec,
    
    /// Total authentication events (labeled by type and success status)
    pub auth_events_total: CounterVec,
    
    /// Total game events (labeled by type: created, completed, abandoned)
    pub game_events_total: CounterVec,
}

impl Metrics {
    /// Create a new Metrics instance with all metrics registered
    pub fn new() -> Self {
        let active_games = Gauge::new(
            "xlmate_active_games",
            "Current number of active games"
        ).expect("Failed to create active_games gauge");
        
        let ws_connections = Gauge::new(
            "xlmate_ws_connections",
            "Current number of active WebSocket connections"
        ).expect("Failed to create ws_connections gauge");
        
        let db_query_duration = Histogram::with_opts(
            HistogramOpts::new(
                "xlmate_db_query_duration_seconds",
                "Database query duration in seconds"
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0])
        ).expect("Failed to create db_query_duration histogram");
        
        let matchmaking_queue_size = Gauge::new(
            "xlmate_matchmaking_queue_size",
            "Number of players waiting in matchmaking queue"
        ).expect("Failed to create matchmaking_queue_size gauge");
        
        let ai_requests_total = CounterVec::new(
            Opts::new(
                "xlmate_ai_requests_total",
                "Total number of AI requests"
            ),
            &["request_type"]
        ).expect("Failed to create ai_requests_total counter");
        
        let auth_events_total = CounterVec::new(
            Opts::new(
                "xlmate_auth_events_total",
                "Total number of authentication events"
            ),
            &["event_type", "success"]
        ).expect("Failed to create auth_events_total counter");
        
        let game_events_total = CounterVec::new(
            Opts::new(
                "xlmate_game_events_total",
                "Total number of game events"
            ),
            &["event_type"]
        ).expect("Failed to create game_events_total counter");
        
        // Register all metrics (only once)
        METRICS_REGISTERED.get_or_init(|| {
            REGISTRY.register(Box::new(active_games.clone())).expect("Failed to register active_games");
            REGISTRY.register(Box::new(ws_connections.clone())).expect("Failed to register ws_connections");
            REGISTRY.register(Box::new(db_query_duration.clone())).expect("Failed to register db_query_duration");
            REGISTRY.register(Box::new(matchmaking_queue_size.clone())).expect("Failed to register matchmaking_queue_size");
            REGISTRY.register(Box::new(ai_requests_total.clone())).expect("Failed to register ai_requests_total");
            REGISTRY.register(Box::new(auth_events_total.clone())).expect("Failed to register auth_events_total");
            REGISTRY.register(Box::new(game_events_total.clone())).expect("Failed to register game_events_total");
            true
        });
        
        Metrics {
            active_games,
            ws_connections,
            db_query_duration,
            matchmaking_queue_size,
            ai_requests_total,
            auth_events_total,
            game_events_total,
        }
    }
    
    /// Get the global registry
    pub fn registry() -> &'static Registry {
        &REGISTRY
    }
}

/// Initialize metrics and return shared instance (idempotent)
pub fn init_metrics() -> Arc<Metrics> {
    GLOBAL_METRICS.get_or_init(|| {
        Arc::new(Metrics::new())
    }).clone()
}

/// Get global metrics instance
fn get_global_metrics() -> Option<&'static Arc<Metrics>> {
    GLOBAL_METRICS.get()
}

/// Increment active games counter
pub fn increment_active_games() {
    if let Some(metrics) = get_global_metrics() {
        metrics.active_games.inc();
    }
}

/// Decrement active games counter
pub fn decrement_active_games() {
    if let Some(metrics) = get_global_metrics() {
        metrics.active_games.dec();
    }
}

/// Increment WebSocket connections counter
pub fn increment_ws_connections() {
    if let Some(metrics) = get_global_metrics() {
        metrics.ws_connections.inc();
    }
}

/// Decrement WebSocket connections counter
pub fn decrement_ws_connections() {
    if let Some(metrics) = get_global_metrics() {
        metrics.ws_connections.dec();
    }
}

/// Set WebSocket connections to specific value
pub fn set_ws_connections(count: i64) {
    if let Some(metrics) = get_global_metrics() {
        metrics.ws_connections.set(count as f64);
    }
}

/// Observe database query duration
pub fn observe_db_query_duration(duration: f64) {
    if let Some(metrics) = get_global_metrics() {
        metrics.db_query_duration.observe(duration);
    }
}

/// Set matchmaking queue size
pub fn set_matchmaking_queue_size(size: i64) {
    if let Some(metrics) = get_global_metrics() {
        metrics.matchmaking_queue_size.set(size as f64);
    }
}

/// Increment matchmaking queue size
pub fn increment_matchmaking_queue() {
    if let Some(metrics) = get_global_metrics() {
        metrics.matchmaking_queue_size.inc();
    }
}

/// Decrement matchmaking queue size
pub fn decrement_matchmaking_queue() {
    if let Some(metrics) = get_global_metrics() {
        metrics.matchmaking_queue_size.dec();
    }
}

/// Increment AI requests counter
pub fn increment_ai_requests(request_type: &str) {
    if let Some(metrics) = get_global_metrics() {
        metrics.ai_requests_total
            .with_label_values(&[request_type])
            .inc();
    }
}

/// Increment authentication events counter
pub fn increment_auth_events(event_type: &str, success: bool) {
    if let Some(metrics) = get_global_metrics() {
        metrics.auth_events_total
            .with_label_values(&[event_type, if success { "true" } else { "false" }])
            .inc();
    }
}

/// Increment game events counter
pub fn increment_game_events(event_type: &str) {
    if let Some(metrics) = get_global_metrics() {
        metrics.game_events_total
            .with_label_values(&[event_type])
            .inc();
    }
}

/// Metrics endpoint handler - returns Prometheus-formatted metrics
pub async fn metrics_handler() -> HttpResponse {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => {
            HttpResponse::Ok()
                .content_type("text/plain; version=0.0.4; charset=utf-8")
                .body(buffer)
        }
        Err(e) => {
            HttpResponse::InternalServerError()
                .body(format!("Failed to encode metrics: {}", e))
        }
    }
}
