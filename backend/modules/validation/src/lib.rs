use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use redis::Client as RedisClient;
use futures_util::StreamExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveValidationRequest {
    pub game_id: Uuid,
    pub player_id: Uuid,
    pub move_notation: String,
    pub timestamp: DateTime<Utc>,
    pub client_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveValidationResponse {
    pub is_valid: bool,
    pub error_message: Option<String>,
    pub processed_move: Option<ProcessedMove>,
    pub validation_time_ms: u64,
    pub engine_suggestion: Option<EngineSuggestion>,
    pub position_analysis: Option<PositionAnalysis>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedMove {
    pub from_square: String,
    pub to_square: String,
    pub piece: String,
    pub is_capture: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
    pub is_castling: bool,
    pub is_en_passant: bool,
    pub promotion_piece: Option<String>,
    pub san: String,
    pub uci: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSuggestion {
    pub best_move: String,
    pub evaluation: f64,
    pub depth: u8,
    pub nodes: u64,
    pub time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionAnalysis {
    pub fen: String,
    pub evaluation: f64,
    pub best_moves: Vec<String>,
    pub mate_in: Option<u8>,
    pub tactical_features: TacticalFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TacticalFeatures {
    pub checks: u8,
    pub captures: u8,
    pub threats: u8,
    pub forks: u8,
    pub pins: u8,
    pub skewers: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCache {
    pub position_fen: String,
    pub move_san: String,
    pub is_valid: bool,
    pub processed_move: ProcessedMove,
    pub cached_at: DateTime<Utc>,
    pub ttl_seconds: u64,
}

#[async_trait]
pub trait MoveValidator: Send + Sync {
    async fn validate_move(&self, request: MoveValidationRequest) -> Result<MoveValidationResponse, ValidationError>;
    async fn batch_validate(&self, requests: Vec<MoveValidationRequest>) -> Vec<Result<MoveValidationResponse, ValidationError>>;
    async fn get_position_analysis(&self, fen: &str, depth: u8) -> Result<PositionAnalysis, ValidationError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid move notation: {0}")]
    InvalidNotation(String),
    
    #[error("Move not legal in current position")]
    IllegalMove,
    
    #[error("Game not found: {0}")]
    GameNotFound(Uuid),
    
    #[error("Player not in game: {0}")]
    PlayerNotInGame(Uuid),
    
    #[error("Not player's turn")]
    NotPlayerTurn,
    
    #[error("Engine validation failed: {0}")]
    EngineError(String),
    
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Internal server error: {0}")]
    InternalError(String),
}

pub struct RealTimeMoveValidator {
    redis_client: RedisClient,
    cache_ttl_seconds: u64,
    max_batch_size: usize,
    rate_limit_per_minute: u32,
    position_cache: HashMap<String, ValidationCache>,
}

impl RealTimeMoveValidator {
    pub fn new(redis_url: &str) -> Self {
        let redis_client = redis::Client::open(redis_url)
            .expect("Failed to create Redis client");
        
        Self {
            redis_client,
            cache_ttl_seconds: 300, // 5 minutes
            max_batch_size: 100,
            rate_limit_per_minute: 60,
            position_cache: HashMap::new(),
        }
    }

    pub fn with_config(
        redis_url: &str,
        cache_ttl_seconds: u64,
        max_batch_size: usize,
        rate_limit_per_minute: u32,
    ) -> Self {
        let redis_client = redis::Client::open(redis_url)
            .expect("Failed to create Redis client");
        
        Self {
            redis_client,
            cache_ttl_seconds,
            max_batch_size,
            rate_limit_per_minute,
            position_cache: HashMap::new(),
        }
    }

    async fn check_rate_limit(&self, player_id: Uuid) -> Result<(), ValidationError> {
        let mut conn = self.redis_client.get_async_connection().await
            .map_err(|e| ValidationError::CacheError(e.to_string()))?;

        let key = format!("rate_limit:{}", player_id);
        let current_count: u32 = conn.get(&key).await.unwrap_or(0);

        if current_count >= self.rate_limit_per_minute {
            return Err(ValidationError::RateLimitExceeded);
        }

        // Increment counter with expiration
        let _: () = conn.incr(&key, 1).await
            .map_err(|e| ValidationError::CacheError(e.to_string()))?;
        
        let _: () = conn.expire(&key, 60).await
            .map_err(|e| ValidationError::CacheError(e.to_string()))?;

        Ok(())
    }

    async fn get_cached_validation(&self, fen: &str, move_san: &str) -> Option<ValidationCache> {
        let cache_key = format!("{}:{}", fen, move_san);
        
        if let Some(cached) = self.position_cache.get(&cache_key) {
            let now = Utc::now();
            let age = (now - cached.cached_at).num_seconds() as u64;
            
            if age < cached.ttl_seconds {
                return Some(cached.clone());
            } else {
                // Remove expired cache entry
                self.position_cache.remove(&cache_key);
            }
        }

        // Check Redis cache
        if let Ok(mut conn) = self.redis_client.get_async_connection().await {
            if let Ok(cached_str) = conn.get::<_, String>(&cache_key).await {
                if let Ok(cached) = serde_json::from_str::<ValidationCache>(&cached_str) {
                    let now = Utc::now();
                    let age = (now - cached.cached_at).num_seconds() as u64;
                    
                    if age < cached.ttl_seconds {
                        // Update local cache
                        self.position_cache.insert(cache_key, cached.clone());
                        return Some(cached);
                    }
                }
            }
        }

        None
    }

    async fn cache_validation(&self, fen: &str, move_san: &str, validation: &ValidationCache) {
        let cache_key = format!("{}:{}", fen, move_san);
        
        // Update local cache
        self.position_cache.insert(cache_key.clone(), validation.clone());
        
        // Update Redis cache
        if let Ok(mut conn) = self.redis_client.get_async_connection().await {
            if let Ok(cached_str) = serde_json::to_string(validation) {
                let _: () = conn.set_ex(&cache_key, cached_str, validation.ttl_seconds).await.unwrap_or(());
            }
        }
    }

    async fn process_move_notation(&self, notation: &str) -> Result<ProcessedMove, ValidationError> {
        // Parse the move notation (SAN or UCI)
        let processed = if notation.len() == 4 && notation.chars().nth(2) == Some(' ') {
            // UCI format (e.g., "e2e4")
            self.parse_uci_move(notation)?
        } else {
            // SAN format (e.g., "e4", "Nf3", "O-O")
            self.parse_san_move(notation)?
        };

        Ok(processed)
    }

    fn parse_uci_move(&self, uci: &str) -> Result<ProcessedMove, ValidationError> {
        if uci.len() < 4 {
            return Err(ValidationError::InvalidNotation("UCI move too short".to_string()));
        }

        let from_square = uci[..2].to_string();
        let to_square = uci[2..4].to_string();
        let promotion_piece = if uci.len() > 4 {
            Some(uci[4..].to_string())
        } else {
            None
        };

        Ok(ProcessedMove {
            from_square,
            to_square,
            piece: "P".to_string(), // Will be determined by position
            is_capture: false,      // Will be determined by position
            is_check: false,        // Will be determined by position
            is_checkmate: false,    // Will be determined by position
            is_castling: from_square.chars().nth(1) == Some('1') && to_square.chars().nth(1) == Some('1') ||
                           from_square.chars().nth(1) == Some('8') && to_square.chars().nth(1) == Some('8'),
            is_en_passant: false,   // Will be determined by position
            promotion_piece,
            san: uci.to_string(),
            uci: uci.to_string(),
        })
    }

    fn parse_san_move(&self, san: &str) -> Result<ProcessedMove, ValidationError> {
        // Basic SAN parsing - this would be enhanced with a proper chess library
        if san == "O-O" {
            return Ok(ProcessedMove {
                from_square: "e1".to_string(),
                to_square: "g1".to_string(),
                piece: "K".to_string(),
                is_capture: false,
                is_check: false,
                is_checkmate: false,
                is_castling: true,
                is_en_passant: false,
                promotion_piece: None,
                san: san.to_string(),
                uci: "e1g1".to_string(),
            });
        }

        if san == "O-O-O" {
            return Ok(ProcessedMove {
                from_square: "e1".to_string(),
                to_square: "c1".to_string(),
                piece: "K".to_string(),
                is_capture: false,
                is_check: false,
                is_checkmate: false,
                is_castling: true,
                is_en_passant: false,
                promotion_piece: None,
                san: san.to_string(),
                uci: "e1c1".to_string(),
            });
        }

        // Parse standard moves (simplified)
        let piece = if san.chars().next().unwrap().is_uppercase() {
            san.chars().next().unwrap().to_string()
        } else {
            "P".to_string()
        };

        let is_check = san.ends_with('+');
        let is_checkmate = san.ends_with('#');
        let is_capture = san.contains('x');

        // Extract destination square (simplified)
        let mut chars = san.chars().rev();
        let rank = chars.next().unwrap();
        let file = chars.next().unwrap();
        let to_square = format!("{}{}", file, rank);

        // This is a simplified implementation
        Ok(ProcessedMove {
            from_square: "?".to_string(), // Will be determined by position
            to_square,
            piece,
            is_capture,
            is_check,
            is_checkmate,
            is_castling: false,
            is_en_passant: san.to_lowercase().contains("ep"),
            promotion_piece: if san.contains('=') {
                san.split('=').nth(1).map(|s| s.to_string())
            } else {
                None
            },
            san: san.to_string(),
            uci: "?".to_string(), // Will be converted after position analysis
        })
    }

    async fn analyze_position(&self, fen: &str, depth: u8) -> Result<PositionAnalysis, ValidationError> {
        // This would integrate with a chess engine
        // For now, return a placeholder implementation
        
        let tactical_features = TacticalFeatures {
            checks: 0,
            captures: 0,
            threats: 0,
            forks: 0,
            pins: 0,
            skewers: 0,
        };

        Ok(PositionAnalysis {
            fen: fen.to_string(),
            evaluation: 0.0,
            best_moves: vec![],
            mate_in: None,
            tactical_features,
        })
    }
}

#[async_trait]
impl MoveValidator for RealTimeMoveValidator {
    async fn validate_move(&self, request: MoveValidationRequest) -> Result<MoveValidationResponse, ValidationError> {
        let start_time = std::time::Instant::now();

        // Check rate limit
        self.check_rate_limit(request.player_id).await?;

        // Get current game position (this would fetch from database)
        let current_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"; // Placeholder

        // Check cache first
        if let Some(cached) = self.get_cached_validation(&current_fen, &request.move_notation).await {
            return Ok(MoveValidationResponse {
                is_valid: cached.is_valid,
                error_message: None,
                processed_move: Some(cached.processed_move),
                validation_time_ms: start_time.elapsed().as_millis() as u64,
                engine_suggestion: None,
                position_analysis: None,
            });
        }

        // Process the move notation
        let processed_move = self.process_move_notation(&request.move_notation).await?;

        // Validate move legality (this would use a proper chess engine)
        let is_valid = self.validate_move_legality(&current_fen, &processed_move).await?;

        let validation_time = start_time.elapsed().as_millis() as u64;

        // Cache the result
        let cache_entry = ValidationCache {
            position_fen: current_fen.to_string(),
            move_san: request.move_notation.clone(),
            is_valid,
            processed_move: processed_move.clone(),
            cached_at: Utc::now(),
            ttl_seconds: self.cache_ttl_seconds,
        };

        self.cache_validation(&current_fen, &request.move_notation, &cache_entry).await;

        Ok(MoveValidationResponse {
            is_valid,
            error_message: if !is_valid { Some("Illegal move".to_string()) } else { None },
            processed_move: Some(processed_move),
            validation_time_ms: validation_time,
            engine_suggestion: None, // Could be added for AI assistance
            position_analysis: None, // Could be added for position analysis
        })
    }

    async fn batch_validate(&self, requests: Vec<MoveValidationRequest>) -> Vec<Result<MoveValidationResponse, ValidationError>> {
        let mut results = Vec::new();

        // Process in batches to avoid overwhelming the system
        for chunk in requests.chunks(self.max_batch_size) {
            let mut batch_results = Vec::new();
            
            for request in chunk {
                let result = self.validate_move(request.clone()).await;
                batch_results.push(result);
            }
            
            results.extend(batch_results);
        }

        results
    }

    async fn get_position_analysis(&self, fen: &str, depth: u8) -> Result<PositionAnalysis, ValidationError> {
        self.analyze_position(fen, depth).await
    }
}

impl RealTimeMoveValidator {
    async fn validate_move_legality(&self, fen: &str, processed_move: &ProcessedMove) -> Result<bool, ValidationError> {
        // This would integrate with a proper chess engine to validate move legality
        // For now, return a placeholder implementation
        
        // Basic validation checks
        if processed_move.from_square == "?" || processed_move.to_square == "?" {
            return Ok(false);
        }

        // In a real implementation, this would:
        // 1. Parse the FEN to create a board position
        // 2. Apply the move to see if it's legal
        // 3. Check for special cases (castling, en passant, promotion)
        // 4. Verify the move doesn't leave the king in check
        
        Ok(true) // Placeholder - always return true for now
    }
}

// WebSocket handler for real-time validation
pub struct ValidationWebSocketHandler {
    validator: Arc<RealTimeMoveValidator>,
}

impl ValidationWebSocketHandler {
    pub fn new(validator: Arc<RealTimeMoveValidator>) -> Self {
        Self { validator }
    }

    pub async fn handle_websocket(&self, stream: actix_ws::MessageStream, game_id: Uuid, player_id: Uuid) {
        use futures_util::StreamExt;
        
        let mut stream = stream;
        while let Some(msg_result) = stream.next().await {
            match msg_result {
                Ok(msg) => {
                    if let actix_ws::Message::Text(text) = msg {
                        if let Ok(request) = serde_json::from_str::<MoveValidationRequest>(&text) {
                            match self.validator.validate_move(request).await {
                                Ok(response) => {
                                    if let Ok(response_text) = serde_json::to_string(&response) {
                                        // In actix-ws 0.4, messages are handled differently
                                        log::info!("Validation response: {}", response_text);
                                    }
                                }
                                Err(e) => {
                                    let error_response = serde_json::json!({
                                        "error": e.to_string()
                                    });
                                    if let Ok(error_text) = serde_json::to_string(&error_response) {
                                        log::error!("Validation error: {}", error_text);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("WebSocket error: {}", e);
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci_move() {
        let validator = RealTimeMoveValidator::new("redis://localhost:6379");
        let result = validator.parse_uci_move("e2e4").unwrap();
        
        assert_eq!(result.from_square, "e2");
        assert_eq!(result.to_square, "e4");
        assert_eq!(result.san, "e2e4");
        assert_eq!(result.uci, "e2e4");
    }

    #[test]
    fn test_parse_san_move() {
        let validator = RealTimeMoveValidator::new("redis://localhost:6379");
        let result = validator.parse_san_move("e4").unwrap();
        
        assert_eq!(result.piece, "P");
        assert_eq!(result.to_square, "e4");
        assert_eq!(result.san, "e4");
        assert!(!result.is_capture);
    }

    #[test]
    fn test_parse_castling() {
        let validator = RealTimeMoveValidator::new("redis://localhost:6379");
        let result = validator.parse_san_move("O-O").unwrap();
        
        assert_eq!(result.piece, "K");
        assert!(result.is_castling);
        assert_eq!(result.from_square, "e1");
        assert_eq!(result.to_square, "g1");
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let validator = RealTimeMoveValidator::with_config("redis://localhost:6379", 300, 100, 2);
        let player_id = Uuid::new_v4();
        
        let request = MoveValidationRequest {
            game_id: Uuid::new_v4(),
            player_id,
            move_notation: "e4".to_string(),
            timestamp: Utc::now(),
            client_version: None,
        };

        // First request should succeed
        assert!(validator.check_rate_limit(player_id).await.is_ok());
        
        // Second request should succeed
        assert!(validator.check_rate_limit(player_id).await.is_ok());
        
        // Third request should fail due to rate limit
        assert!(matches!(validator.check_rate_limit(player_id).await, Err(ValidationError::RateLimitExceeded)));
    }
}
