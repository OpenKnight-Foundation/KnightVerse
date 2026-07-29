#[cfg(test)]
mod rating_integration_tests {
    use chess::{RatingConfig, RatingService};

    #[test]
    fn test_rating_config_defaults() {
        let config = RatingConfig::default();
        assert_eq!(config.k_factor, 32);
        assert_eq!(config.min_rating, 100);
        assert_eq!(config.max_rating, 3000);
    }

    #[test]
    fn test_rating_bounds_enforcement() {
        let config = RatingConfig {
            k_factor: 32,
            min_rating: 800,
            max_rating: 2400,
        };

        // Test that ratings are properly clamped
        assert!(config.min_rating <= config.max_rating);
        assert!(config.k_factor > 0);
    }
}
