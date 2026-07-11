use prometheus::{Counter, Histogram, Gauge, Registry, TextEncoder, Encoder};
use actix_web::{HttpResponse, web};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Custom metrics collector for XLMate
pub struct MetricsCollector {
    registry: Registry,
    
    // HTTP metrics
    pub http_requests_total: Counter,
    pub http_request_duration: Histogram,
    
    // Game metrics
    pub games_created_total: Counter,
    pub games_completed_total: Counter,
    pub active_games: Gauge,
    pub moves_made_total: Counter,
    
    // User metrics
    pub users_registered_total: Counter,
    pub active_users: Gauge,
    
    // Tournament metrics
    pub tournaments_created_total: Counter,
    pub active_tournaments: Gauge,
    pub tournament_participants: Gauge,
    
    // System metrics
    pub database_connections: Gauge,
    pub websocket_connections: Gauge,
}

impl MetricsCollector {
    pub fn new() -> Self {
        let registry = Registry::new();
        
        // HTTP metrics
        let http_requests_total = Counter::new(
            "http_requests_total",
            "Total number of HTTP requests"
        ).unwrap();
        
        let http_request_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds"
            ).buckets(vec![0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0])
        ).unwrap();
        
        // Game metrics
        let games_created_total = Counter::new(
            "games_created_total",
            "Total number of games created"
        ).unwrap();
        
        let games_completed_total = Counter::new(
            "games_completed_total",
            "Total number of games completed"
        ).unwrap();
        
        let active_games = Gauge::new(
            "active_games",
            "Number of currently active games"
        ).unwrap();
        
        let moves_made_total = Counter::new(
            "moves_made_total",
            "Total number of moves made across all games"
        ).unwrap();
        
        // User metrics
        let users_registered_total = Counter::new(
            "users_registered_total",
            "Total number of registered users"
        ).unwrap();
        
        let active_users = Gauge::new(
            "active_users",
            "Number of currently active users"
        ).unwrap();
        
        // Tournament metrics
        let tournaments_created_total = Counter::new(
            "tournaments_created_total",
            "Total number of tournaments created"
        ).unwrap();
        
        let active_tournaments = Gauge::new(
            "active_tournaments",
            "Number of currently active tournaments"
        ).unwrap();
        
        let tournament_participants = Gauge::new(
            "tournament_participants",
            "Number of participants in active tournaments"
        ).unwrap();
        
        // System metrics
        let database_connections = Gauge::new(
            "database_connections",
            "Number of active database connections"
        ).unwrap();
        
        let websocket_connections = Gauge::new(
            "websocket_connections",
            "Number of active WebSocket connections"
        ).unwrap();
        
        // Register all metrics
        registry.register(Box::new(http_requests_total.clone())).unwrap();
        registry.register(Box::new(http_request_duration.clone())).unwrap();
        registry.register(Box::new(games_created_total.clone())).unwrap();
        registry.register(Box::new(games_completed_total.clone())).unwrap();
        registry.register(Box::new(active_games.clone())).unwrap();
        registry.register(Box::new(moves_made_total.clone())).unwrap();
        registry.register(Box::new(users_registered_total.clone())).unwrap();
        registry.register(Box::new(active_users.clone())).unwrap();
        registry.register(Box::new(tournaments_created_total.clone())).unwrap();
        registry.register(Box::new(active_tournaments.clone())).unwrap();
        registry.register(Box::new(tournament_participants.clone())).unwrap();
        registry.register(Box::new(database_connections.clone())).unwrap();
        registry.register(Box::new(websocket_connections.clone())).unwrap();
        
        Self {
            registry,
            http_requests_total,
            http_request_duration,
            games_created_total,
            games_completed_total,
            active_games,
            moves_made_total,
            users_registered_total,
            active_users,
            tournaments_created_total,
            active_tournaments,
            tournament_participants,
            database_connections,
            websocket_connections,
        }
    }
    
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
    
    /// Export metrics in Prometheus format
    pub fn export(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        encoder.encode_to_string(&metric_families)
    }
}

/// Middleware metrics trait
pub trait MiddlewareMetrics {
    fn inc_http_requests(&self, method: &str, path: &str, status: u16);
    fn observe_http_duration(&self, method: &str, path: &str, duration: f64);
}

impl MiddlewareMetrics for MetricsCollector {
    fn inc_http_requests(&self, method: &str, path: &str, status: u16) {
        self.http_requests_total
            .with_label_values(&[method, path, &status.to_string()])
            .inc();
    }
    
    fn observe_http_duration(&self, method: &str, path: &str, duration: f64) {
        self.http_request_duration
            .with_label_values(&[method, path])
            .observe(duration);
    }
}

/// Game-specific metrics
pub trait GameMetrics {
    fn inc_games_created(&self);
    fn inc_games_completed(&self, result: &str);
    fn inc_moves_made(&self);
    fn set_active_games(&self, count: f64);
}

impl GameMetrics for MetricsCollector {
    fn inc_games_created(&self) {
        self.games_created_total.inc();
    }
    
    fn inc_games_completed(&self, result: &str) {
        self.games_completed_total
            .with_label_values(&[result])
            .inc();
    }
    
    fn inc_moves_made(&self) {
        self.moves_made_total.inc();
    }
    
    fn set_active_games(&self, count: f64) {
        self.active_games.set(count);
    }
}

/// User-specific metrics
pub trait UserMetrics {
    fn inc_users_registered(&self);
    fn set_active_users(&self, count: f64);
}

impl UserMetrics for MetricsCollector {
    fn inc_users_registered(&self) {
        self.users_registered_total.inc();
    }
    
    fn set_active_users(&self, count: f64) {
        self.active_users.set(count);
    }
}

/// Tournament-specific metrics
pub trait TournamentMetrics {
    fn inc_tournaments_created(&self);
    fn set_active_tournaments(&self, count: f64);
    fn set_tournament_participants(&self, count: f64);
}

impl TournamentMetrics for MetricsCollector {
    fn inc_tournaments_created(&self) {
        self.tournaments_created_total.inc();
    }
    
    fn set_active_tournaments(&self, count: f64) {
        self.active_tournaments.set(count);
    }
    
    fn set_tournament_participants(&self, count: f64) {
        self.tournament_participants.set(count);
    }
}

/// System-specific metrics
pub trait SystemMetrics {
    fn set_database_connections(&self, count: f64);
    fn set_websocket_connections(&self, count: f64);
}

impl SystemMetrics for MetricsCollector {
    fn set_database_connections(&self, count: f64) {
        self.database_connections.set(count);
    }
    
    fn set_websocket_connections(&self, count: f64) {
        self.websocket_connections.set(count);
    }
}

/// Actix-web middleware for metrics collection
pub async fn metrics_endpoint(metrics: web::Data<Arc<MetricsCollector>>) -> HttpResponse {
    match metrics.export() {
        Ok(metrics_text) => HttpResponse::Ok()
            .content_type("text/plain; version=0.0.4; charset=utf-8")
            .body(metrics_text),
        Err(e) => {
            log::error!("Failed to export metrics: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        
        // Test that all metrics are initialized
        collector.http_requests_total.inc();
        collector.games_created_total.inc();
        collector.users_registered_total.inc();
        
        assert!(collector.export().is_ok());
    }
    
    #[test]
    fn test_metrics_export() {
        let collector = MetricsCollector::new();
        
        // Increment some metrics
        collector.http_requests_total.inc();
        collector.games_created_total.inc();
        
        let exported = collector.export().unwrap();
        assert!(exported.contains("http_requests_total"));
        assert!(exported.contains("games_created_total"));
    }
}
