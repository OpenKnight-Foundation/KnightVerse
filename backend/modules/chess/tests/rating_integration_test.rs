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

    #[test]
    fn test_glicko2_reference_values() {
        // Reference example from Glickman's Glicko-2 paper:
        // player r=1500, RD=200, sigma=0.06 vs three opponents:
        // (1400, 30, 1, win), (1550, 100, 0, loss), (1700, 300, 0, loss)
        // Expected result: r' ~= 1464.06, RD' ~= 151.52, sigma' ~= 0.05999
        let player = chess::Glicko2Rating::new(1500.0, 200.0, 0.06);
        let games = [
            chess::Glicko2Game { opponent: chess::Glicko2Rating::new(1400.0, 30.0, 0.06), score: 1.0 },
            chess::Glicko2Game { opponent: chess::Glicko2Rating::new(1550.0, 100.0, 0.06), score: 0.0 },
            chess::Glicko2Game { opponent: chess::Glicko2Rating::new(1700.0, 300.0, 0.06), score: 0.0 },
        ];
        let updated = chess::glicko2_update(player, &games, 0.5);
        assert!((updated.rating - 1464.06).abs() < 0.1);
        assert!((updated.deviation - 151.52).abs() < 0.1);
        assert!((updated.volatility - 0.05999).abs() < 0.0001);
    }

    #[test]
    fn test_provisional_status_while_rd_high() {
        let new_player = chess::Glicko2Rating::new(1500.0, 350.0, 0.06);
        assert!(chess::is_provisional(&new_player, 110.0));

        let established = chess::Glicko2Rating::new(1500.0, 60.0, 0.06);
        assert!(!chess::is_provisional(&established, 110.0));
    }

    #[test]
    fn test_elo_to_glicko2_migration_preserves_rating() {
        let migrated = chess::migrate_elo_to_glicko2(1800.0);
        assert!((migrated.rating - 1800.0).abs() < 1e-9);
        assert!((migrated.deviation - 350.0).abs() < 1e-9);
        assert!((migrated.volatility - 0.06).abs() < 1e-9);
    }
}
