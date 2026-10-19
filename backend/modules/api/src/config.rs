
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_zero_rate_limit() {
        let config = AppConfig {
            auth_rate_limit_per_sec: 0,
            auth_rate_limit_burst: 5,
            game_rate_limit_per_sec: 10,
            game_rate_limit_burst: 20,
            redis_auth_rate_limit: 5,
            redis_auth_rate_limit_window: 60,
            redis_game_rate_limit: 30,
            redis_game_rate_limit_window: 60,
        };

        let error = config.validate().unwrap_err();

        assert_eq!(error.variable, "AUTH_RATE_LIMIT_PER_SEC");
        assert_eq!(error.reason, "must be greater than zero");
    }

    #[test]
    fn accepts_valid_configuration() {
        let config = AppConfig {
            auth_rate_limit_per_sec: 1,
            auth_rate_limit_burst: 5,
            game_rate_limit_per_sec: 10,
            game_rate_limit_burst: 20,
            redis_auth_rate_limit: 5,
            redis_auth_rate_limit_window: 60,
            redis_game_rate_limit: 30,
            redis_game_rate_limit_window: 60,
        };

        assert!(config.validate().is_ok());
    }
}
