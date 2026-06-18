use sea_orm::{DatabaseConnection, DatabaseTransaction, DbErr, TransactionTrait, EntityTrait, ActiveModelTrait, Set};
use uuid::Uuid;
use db_entity::{player, game};
use error::error::ApiError;
use matchmaking::elo::calculate_new_ratings;

/// Service for handling Elo rating calculations and updates after game completion
pub struct RatingService;

/// Represents the outcome of a game from a player's perspective
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameOutcome {
    Win,
    Loss,
    Draw,
}

/// Configuration for Elo rating calculations
#[derive(Debug, Clone)]
pub struct RatingConfig {
    /// K-factor for rating calculations (typically 32 for new players, 16 for experienced)
    pub k_factor: u32,
    /// Minimum rating (prevents ratings from going below this value)
    pub min_rating: i32,
    /// Maximum rating (prevents ratings from going above this value)  
    pub max_rating: i32,
}

impl Default for RatingConfig {
    fn default() -> Self {
        Self {
            k_factor: 32,
            min_rating: 100,
            max_rating: 3000,
        }
    }
}

impl RatingService {
    /// Updates player ratings after a game completion using a database transaction
    /// 
    /// # Arguments
    /// * `db` - Database connection
    /// * `game_id` - UUID of the completed game
    /// * `config` - Rating configuration (K-factor, min/max ratings)
    /// 
    /// # Returns
    /// * `Ok((white_new_rating, black_new_rating))` - New ratings for both players
    /// * `Err(ApiError)` - If game not found, players not found, or database error
    /// 
    /// # Example
    /// ```rust
    /// let config = RatingConfig::default();
    /// let (white_rating, black_rating) = RatingService::update_ratings_after_game(
    ///     &db, 
    ///     game_id, 
    ///     &config
    /// ).await?;
    /// ```
    pub async fn update_ratings_after_game(
        db: &DatabaseConnection,
        game_id: Uuid,
        config: &RatingConfig,
    ) -> Result<(i32, i32), ApiError> {
        // Start a database transaction to ensure atomicity
        let txn = db.begin().await
            .map_err(ApiError::DatabaseError)?;

        let result = Self::update_ratings_in_transaction(&txn, game_id, config).await;

        match result {
            Ok(ratings) => {
                // Commit the transaction if everything succeeded
                txn.commit().await
                    .map_err(ApiError::DatabaseError)?;
                Ok(ratings)
            }
            Err(e) => {
                // Rollback the transaction on any error
                let _ = txn.rollback().await; // Ignore rollback errors
                Err(e)
            }
        }
    }

    /// Internal method that performs the rating update within a transaction
    pub async fn update_ratings_in_transaction(
        txn: &DatabaseTransaction,
        game_id: Uuid,
        config: &RatingConfig,
    ) -> Result<(i32, i32), ApiError> {
        // 1. Fetch the game with result
        let game_model = game::Entity::find_by_id(game_id)
            .one(txn)
            .await
            .map_err(ApiError::DatabaseError)?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        // 2. Check if game is completed
        let game_result = game_model.result
            .ok_or_else(|| ApiError::BadRequest("Game is not completed yet".to_string()))?;

        // 3. Determine game outcome
        let (white_outcome, _black_outcome) = match game_result {
            db_entity::game::ResultSide::WhiteWins => (GameOutcome::Win, GameOutcome::Loss),
            db_entity::game::ResultSide::BlackWins => (GameOutcome::Loss, GameOutcome::Win),
            db_entity::game::ResultSide::Draw => (GameOutcome::Draw, GameOutcome::Draw),
            db_entity::game::ResultSide::Ongoing => {
                return Err(ApiError::BadRequest("Game is still ongoing".to_string()));
            }
            db_entity::game::ResultSide::Abandoned => {
                // For abandoned games, we don't update ratings
                return Err(ApiError::BadRequest("Ratings not updated for abandoned games".to_string()));
            }
        };

        // 4. Fetch both players with their current ratings
        let white_player = player::Entity::find_by_id(game_model.white_player)
            .one(txn)
            .await
            .map_err(ApiError::DatabaseError)?
            .ok_or_else(|| ApiError::NotFound("White player not found".to_string()))?;

        let black_player = player::Entity::find_by_id(game_model.black_player)
            .one(txn)
            .await
            .map_err(ApiError::DatabaseError)?
            .ok_or_else(|| ApiError::NotFound("Black player not found".to_string()))?;

        // 5. Calculate new ratings based on game outcome
        let (new_white_rating, new_black_rating) = Self::calculate_rating_changes(
            white_player.elo_rating,
            black_player.elo_rating,
            white_outcome,
            config,
        );

        // 6. Update both players' ratings atomically
        let white_active_model = player::ActiveModel {
            id: Set(white_player.id),
            elo_rating: Set(new_white_rating),
            ..Default::default()
        };

        let black_active_model = player::ActiveModel {
            id: Set(black_player.id),
            elo_rating: Set(new_black_rating),
            ..Default::default()
        };

        // Execute both updates in the same transaction
        white_active_model.update(txn).await
            .map_err(ApiError::DatabaseError)?;

        black_active_model.update(txn).await
            .map_err(ApiError::DatabaseError)?;

        Ok((new_white_rating, new_black_rating))
    }

    /// Calculates new ratings based on game outcome using Elo formula
    fn calculate_rating_changes(
        white_rating: i32,
        black_rating: i32,
        white_outcome: GameOutcome,
        config: &RatingConfig,
    ) -> (i32, i32) {
        let (new_white, new_black) = match white_outcome {
            GameOutcome::Win => {
                // White wins, black loses
                calculate_new_ratings(white_rating as u32, black_rating as u32, config.k_factor)
            }
            GameOutcome::Loss => {
                // White loses, black wins
                let (new_black, new_white) = calculate_new_ratings(black_rating as u32, white_rating as u32, config.k_factor);
                (new_white, new_black)
            }
            GameOutcome::Draw => {
                // Draw: both players get half points
                Self::calculate_draw_ratings(white_rating, black_rating, config.k_factor)
            }
        };

        // Apply rating bounds
        let clamped_white = (new_white as i32).clamp(config.min_rating, config.max_rating);
        let clamped_black = (new_black as i32).clamp(config.min_rating, config.max_rating);

        (clamped_white, clamped_black)
    }

    /// Calculates rating changes for a draw (both players get 0.5 points)
    fn calculate_draw_ratings(white_rating: i32, black_rating: i32, k_factor: u32) -> (u32, u32) {
        let white_f = white_rating as f64;
        let black_f = black_rating as f64;
        let k_f = k_factor as f64;

        // Expected score for white player
        let exponent = (black_f - white_f) / 400.0;
        let expected_white = 1.0 / (1.0 + 10_f64.powf(exponent));

        // In a draw, both players get 0.5 points
        let white_delta = (k_f * (0.5 - expected_white)).round();
        let black_delta = -white_delta; // Zero-sum game

        let new_white = (white_rating as f64 + white_delta).max(0.0) as u32;
        let new_black = (black_rating as f64 + black_delta).max(0.0) as u32;

        (new_white, new_black)
    }

    /// Gets the current rating for a player
    pub async fn get_player_rating(
        db: &DatabaseConnection,
        player_id: Uuid,
    ) -> Result<i32, ApiError> {
        let player = player::Entity::find_by_id(player_id)
            .one(db)
            .await
            .map_err(ApiError::DatabaseError)?
            .ok_or_else(|| ApiError::NotFound("Player not found".to_string()))?;

        Ok(player.elo_rating)
    }

    /// Updates a player's rating directly (for admin purposes or initial setup)
    pub async fn set_player_rating(
        db: &DatabaseConnection,
        player_id: Uuid,
        new_rating: i32,
        config: &RatingConfig,
    ) -> Result<(), ApiError> {
        let clamped_rating = new_rating.clamp(config.min_rating, config.max_rating);

        let active_model = player::ActiveModel {
            id: Set(player_id),
            elo_rating: Set(clamped_rating),
            ..Default::default()
        };

        active_model.update(db).await
            .map_err(ApiError::DatabaseError)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_rating_changes_white_wins() {
        let config = RatingConfig::default();
        let (new_white, new_black) = RatingService::calculate_rating_changes(
            1500, 1500, GameOutcome::Win, &config
        );
        
        // Equal ratings, white wins: white gains ~16, black loses ~16
        assert!(new_white > 1500);
        assert!(new_black < 1500);
        assert_eq!(new_white - 1500, 1500 - new_black); // Zero-sum
    }

    #[test]
    fn test_calculate_rating_changes_draw() {
        let config = RatingConfig::default();
        let (new_white, new_black) = RatingService::calculate_rating_changes(
            1600, 1400, GameOutcome::Draw, &config
        );
        
        // Higher rated player loses points in draw, lower rated gains
        assert!(new_white < 1600);
        assert!(new_black > 1400);
    }

    #[test]
    fn test_rating_bounds() {
        let config = RatingConfig {
            k_factor: 32,
            min_rating: 100,
            max_rating: 2000,
        };
        
        let (new_white, new_black) = RatingService::calculate_rating_changes(
            50, 2500, GameOutcome::Win, &config
        );
        
        // Ratings should be clamped to bounds
        assert!(new_white >= config.min_rating);
        assert!(new_white <= config.max_rating);
        assert!(new_black >= config.min_rating);
        assert!(new_black <= config.max_rating);
    }

    #[test]
    fn test_upset_victory_large_rating_change() {
        let config = RatingConfig::default();
        let (new_white, new_black) = RatingService::calculate_rating_changes(
            1200, 1800, GameOutcome::Win, &config
        );
        
        // Lower rated player beating higher rated should gain significant points
        let white_gain = new_white - 1200;
        let black_loss = 1800 - new_black;
        
        assert!(white_gain > 20); // Significant gain for upset
        assert!(black_loss > 20); // Significant loss for upset
        assert_eq!(white_gain, black_loss); // Zero-sum
    }
}