use std::env;
use std::fmt;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub auth_rate_limit_per_sec: u64,
    pub auth_rate_limit_burst: u32,
    pub game_rate_limit_per_sec: u64,
    pub game_rate_limit_burst: u32,
    pub redis_auth_rate_limit: u64,
    pub redis_auth_rate_limit_window: u64,
    pub redis_game_rate_limit: u64,
    pub redis_game_rate_limit_window: u64,
    /// BE-137: `/v1/ai` is the most CPU-expensive scope in the API, so it gets
    /// its own (tighter) budget rather than borrowing the games one.
    pub ai_rate_limit_per_sec: u64,
    pub ai_rate_limit_burst: u32,
    pub redis_ai_rate_limit: u64,
    pub redis_ai_rate_limit_window: u64,
}

/// A configuration value that can't be used, named by its environment variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub variable: &'static str,
    pub reason: &'static str,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.variable, self.reason)
    }
}

impl std::error::Error for ConfigError {}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            auth_rate_limit_per_sec: env::var("AUTH_RATE_LIMIT_PER_SEC")
                .unwrap_or_else(|_| "1".to_string())
                .parse()
                .unwrap_or(1),
            auth_rate_limit_burst: env::var("AUTH_RATE_LIMIT_BURST")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            game_rate_limit_per_sec: env::var("GAME_RATE_LIMIT_PER_SEC")
                .unwrap_or_else(|_| "10".to_string()) // Looser limit for games
                .parse()
                .unwrap_or(10),
            game_rate_limit_burst: env::var("GAME_RATE_LIMIT_BURST")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .unwrap_or(20),
            redis_auth_rate_limit: env::var("REDIS_AUTH_RATE_LIMIT")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .unwrap_or(5),
            redis_auth_rate_limit_window: env::var("REDIS_AUTH_RATE_LIMIT_WINDOW")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            redis_game_rate_limit: env::var("REDIS_GAME_RATE_LIMIT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            redis_game_rate_limit_window: env::var("REDIS_GAME_RATE_LIMIT_WINDOW")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            ai_rate_limit_per_sec: env::var("AI_RATE_LIMIT_PER_SEC")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(2),
            ai_rate_limit_burst: env::var("AI_RATE_LIMIT_BURST")
                .unwrap_or_else(|_| "4".to_string())
                .parse()
                .unwrap_or(4),
            redis_ai_rate_limit: env::var("REDIS_AI_RATE_LIMIT")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            redis_ai_rate_limit_window: env::var("REDIS_AI_RATE_LIMIT_WINDOW")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
        }
    }

    /// Rejects limits that would block every request (or divide by zero in a
    /// rate window), naming the environment variable to fix.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let values: [(&'static str, u64); 12] = [
            ("AUTH_RATE_LIMIT_PER_SEC", self.auth_rate_limit_per_sec),
            ("AUTH_RATE_LIMIT_BURST", u64::from(self.auth_rate_limit_burst)),
            ("GAME_RATE_LIMIT_PER_SEC", self.game_rate_limit_per_sec),
            ("GAME_RATE_LIMIT_BURST", u64::from(self.game_rate_limit_burst)),
            ("REDIS_AUTH_RATE_LIMIT", self.redis_auth_rate_limit),
            ("REDIS_AUTH_RATE_LIMIT_WINDOW", self.redis_auth_rate_limit_window),
            ("REDIS_GAME_RATE_LIMIT", self.redis_game_rate_limit),
            ("REDIS_GAME_RATE_LIMIT_WINDOW", self.redis_game_rate_limit_window),
            ("AI_RATE_LIMIT_PER_SEC", self.ai_rate_limit_per_sec),
            ("AI_RATE_LIMIT_BURST", u64::from(self.ai_rate_limit_burst)),
            ("REDIS_AI_RATE_LIMIT", self.redis_ai_rate_limit),
            ("REDIS_AI_RATE_LIMIT_WINDOW", self.redis_ai_rate_limit_window),
        ];

        for (variable, value) in values {
            if value == 0 {
                return Err(ConfigError {
                    variable,
                    reason: "must be greater than zero",
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> AppConfig {
        AppConfig {
            auth_rate_limit_per_sec: 1,
            auth_rate_limit_burst: 5,
            game_rate_limit_per_sec: 10,
            game_rate_limit_burst: 20,
            redis_auth_rate_limit: 5,
            redis_auth_rate_limit_window: 60,
            redis_game_rate_limit: 30,
            redis_game_rate_limit_window: 60,
            ai_rate_limit_per_sec: 2,
            ai_rate_limit_burst: 4,
            redis_ai_rate_limit: 10,
            redis_ai_rate_limit_window: 60,
        }
    }

    #[test]
    fn rejects_zero_rate_limit() {
        let config = AppConfig {
            auth_rate_limit_per_sec: 0,
            ..valid_config()
        };

        let error = config.validate().unwrap_err();

        assert_eq!(error.variable, "AUTH_RATE_LIMIT_PER_SEC");
        assert_eq!(error.reason, "must be greater than zero");
    }

    #[test]
    fn accepts_valid_configuration() {
        assert!(valid_config().validate().is_ok());
    }
}
