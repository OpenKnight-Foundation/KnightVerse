use std::env;

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
}
