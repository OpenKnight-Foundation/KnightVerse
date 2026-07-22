#[cfg(test)]
mod tests {
    use crate::metrics::{Metrics, init_metrics};
    use prometheus::Encoder;
    use std::sync::Arc;

    // Initialize metrics once for all tests
    fn get_metrics() -> Arc<Metrics> {
        init_metrics()
    }

    #[test]
    fn test_metrics_initialization() {
        // Initialize metrics
        let metrics = get_metrics();
        
        // Verify metrics instance is created
        assert!(metrics.active_games.get() >= 0.0);
        assert!(metrics.ws_connections.get() >= 0.0);
    }

    #[test]
    fn test_active_games_increment_decrement() {
        let metrics = get_metrics();
        
        let initial_value = metrics.active_games.get();
        
        // Increment
        metrics.active_games.inc();
        assert_eq!(metrics.active_games.get(), initial_value + 1.0);
        
        // Decrement
        metrics.active_games.dec();
        assert_eq!(metrics.active_games.get(), initial_value);
    }

    #[test]
    fn test_ws_connections_increment_decrement() {
        let metrics = get_metrics();
        
        let initial_value = metrics.ws_connections.get();
        
        // Increment
        metrics.ws_connections.inc();
        assert_eq!(metrics.ws_connections.get(), initial_value + 1.0);
        
        // Decrement
        metrics.ws_connections.dec();
        assert_eq!(metrics.ws_connections.get(), initial_value);
    }

    #[test]
    fn test_ws_connections_set() {
        let metrics = get_metrics();
        
        // Set to specific value
        metrics.ws_connections.set(42.0);
        assert_eq!(metrics.ws_connections.get(), 42.0);
        
        // Set to another value
        metrics.ws_connections.set(10.0);
        assert_eq!(metrics.ws_connections.get(), 10.0);
    }

    #[test]
    fn test_db_query_duration_histogram() {
        let metrics = get_metrics();
        
        // Observe some durations
        metrics.db_query_duration.observe(0.05);
        metrics.db_query_duration.observe(0.1);
        metrics.db_query_duration.observe(0.15);
        
        // Verify the histogram has samples
        let metric_protos = metrics.db_query_duration.collect();
        assert!(!metric_protos.is_empty());
    }

    #[test]
    fn test_counters_increment() {
        let metrics = get_metrics();
        
        // Increment counters with labels
        metrics.ai_requests_total.with_label_values(&["suggestion"]).inc();
        metrics.auth_events_total.with_label_values(&["login", "true"]).inc();
        metrics.game_events_total.with_label_values(&["created"]).inc();
        
        // Verify counters incremented
        assert_eq!(metrics.ai_requests_total.with_label_values(&["suggestion"]).get(), 1.0);
        assert_eq!(metrics.auth_events_total.with_label_values(&["login", "true"]).get(), 1.0);
        assert_eq!(metrics.game_events_total.with_label_values(&["created"]).get(), 1.0);
    }

    #[test]
    fn test_matchmaking_queue_operations() {
        let metrics = get_metrics();
        
        let initial_value = metrics.matchmaking_queue_size.get();
        
        // Increment queue
        metrics.matchmaking_queue_size.inc();
        assert_eq!(metrics.matchmaking_queue_size.get(), initial_value + 1.0);
        
        // Set to specific value
        metrics.matchmaking_queue_size.set(15.0);
        assert_eq!(metrics.matchmaking_queue_size.get(), 15.0);
        
        // Decrement queue
        metrics.matchmaking_queue_size.dec();
        assert_eq!(metrics.matchmaking_queue_size.get(), 14.0);
    }

    #[test]
    fn test_metrics_registry_gather() {
        let metrics = get_metrics();
        
        // Gather metrics from registry
        let registry = Metrics::registry();
        let metric_families = registry.gather();
        
        // Verify we have metrics registered
        assert!(!metric_families.is_empty());
        
        // Should have at least our 7 custom metrics
        assert!(metric_families.len() >= 7);
    }

    #[test]
    fn test_metrics_encoding() {
        let metrics = get_metrics();
        
        // Perform some operations
        metrics.active_games.inc();
        metrics.ws_connections.set(5.0);
        metrics.ai_requests_total.with_label_values(&["suggestion"]).inc();
        
        // Encode metrics
        let encoder = prometheus::TextEncoder::new();
        let registry = Metrics::registry();
        let metric_families = registry.gather();
        let mut buffer = Vec::new();
        
        let result = encoder.encode(&metric_families, &mut buffer);
        
        // Encoding should succeed
        assert!(result.is_ok());
        
        // Buffer should contain metric data
        assert!(!buffer.is_empty());
        
        // Convert to string and verify format
        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("xlmate_active_games"));
        assert!(output.contains("xlmate_ws_connections"));
        assert!(output.contains("xlmate_ai_requests_total"));
    }

    #[test]
    fn test_concurrent_metric_updates() {
        use std::thread;
        
        let metrics = get_metrics();
        
        // Snapshot counter values before spawning threads
        let initial_active_games = metrics.active_games.get();
        let initial_ai_requests = metrics.ai_requests_total.with_label_values(&["suggestion"]).get();
        
        let mut handles = vec![];
        
        // Spawn multiple threads to increment metrics
        for _ in 0..10 {
            let metrics_clone = metrics.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    metrics_clone.active_games.inc();
                    metrics_clone.ai_requests_total.with_label_values(&["suggestion"]).inc();
                }
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Verify delta is correct (10 threads * 100 increments)
        let delta_active_games = metrics.active_games.get() - initial_active_games;
        let delta_ai_requests = metrics.ai_requests_total.with_label_values(&["suggestion"]).get() - initial_ai_requests;
        
        assert_eq!(delta_active_games, 1000.0);
        assert_eq!(delta_ai_requests, 1000.0);
    }
}
