//! Elo rating updates with a per-time-control K-factor.
//!
//! # K-factor table
//!
//! Faster time controls produce noisier single-game results, so ratings take
//! bigger steps to converge; slower, more decisive games take smaller steps
//! for long-run stability:
//!
//! | Category  | K  | Rationale                                  |
//! |-----------|----|--------------------------------------------|
//! | Bullet    | 56 | Highest noise, fastest convergence         |
//! | Blitz     | 48 | High noise, fast convergence               |
//! | Rapid     | 40 | Moderate noise and decisiveness            |
//! | Classical | 32 | Most decisive, most stable; keeps the historical single K-factor |
//!
//! `Classical` intentionally reuses the pre-existing default K-factor, and a
//! missing time control (`None`) falls back to `RatingConfig::k_factor`, so
//! existing callers and unmapped games behave exactly as before.

use super::time_control::TimeControlCategory;
use db_entity::{game, player};
use error::error::ApiError;
use matchmaking::elo::calculate_new_ratings;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait, Set,
    TransactionTrait,
};
use uuid::Uuid;

/// K-factor for bullet games (noisiest, fastest convergence).
pub const K_FACTOR_BULLET: u32 = 56;
/// K-factor for blitz games.
pub const K_FACTOR_BLITZ: u32 = 48;
/// K-factor for rapid games.
pub const K_FACTOR_RAPID: u32 = 40;
/// K-factor for classical games. Matches the historical single K-factor so
/// default behavior is unchanged for classical and unmapped games.
pub const K_FACTOR_CLASSICAL: u32 = 32;

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
            k_factor: K_FACTOR_CLASSICAL,
            min_rating: 100,
            max_rating: 3000,
        }
    }
}

impl RatingConfig {
    /// Returns the K-factor for a time-control category (see the module-level
    /// table). The classical entry equals the default `k_factor`.
    pub fn k_factor_for_category(category: TimeControlCategory) -> u32 {
        match category {
            TimeControlCategory::Bullet => K_FACTOR_BULLET,
            TimeControlCategory::Blitz => K_FACTOR_BLITZ,
            TimeControlCategory::Rapid => K_FACTOR_RAPID,
            TimeControlCategory::Classical => K_FACTOR_CLASSICAL,
        }
    }

    /// Returns the effective K-factor: the table value when the game's time
    /// control is known, otherwise the configured single `k_factor`
    /// (legacy behavior for callers that don't track time controls).
    pub fn effective_k_factor(&self, time_control: Option<TimeControlCategory>) -> u32 {
        match time_control {
            Some(category) => Self::k_factor_for_category(category),
            None => self.k_factor,
        }
    }
}

impl RatingService {
    /// Updates player ratings after a game completion using a database transaction
    ///
    /// # Arguments
    /// * `db` - Database connection
    /// * `game_id` - UUID of the completed game
    /// * `config` - Rating configuration (K-factor fallback, min/max ratings)
    /// * `time_control` - Game's time-control category, when known. Selects
    ///   the K-factor from the module-level table and takes precedence over
    ///   `config.k_factor`; `None` preserves the legacy single-K behavior.
    ///
    /// # Returns
    /// * `Ok((white_new_rating, black_new_rating))` - New ratings for both players
    /// * `Err(ApiError)` - If game not found, players not found, or database error
    ///
    /// # Example
    /// ```ignore
    /// // Illustrative only — `db` and `game_id` come from the caller's context.
    /// let config = RatingConfig::default();
    /// let (white_rating, black_rating) = RatingService::update_ratings_after_game(
    ///     &db,
    ///     game_id,
    ///     &config,
    ///     None
    /// ).await?;
    /// ```
    pub async fn update_ratings_after_game(
        db: &DatabaseConnection,
        game_id: Uuid,
        config: &RatingConfig,
        time_control: Option<TimeControlCategory>,
    ) -> Result<(i32, i32), ApiError> {
        // Start a database transaction to ensure atomicity
        let txn = db.begin().await.map_err(|e| {
            ApiError::DatabaseError(DbErr::Custom(format!("Failed to start transaction: {}", e)))
        })?;

        let result = Self::update_ratings_in_transaction(&txn, game_id, config, time_control).await;

        match result {
            Ok(ratings) => {
                // Commit the transaction if everything succeeded
                txn.commit().await.map_err(|e| {
                    ApiError::DatabaseError(DbErr::Custom(format!(
                        "Failed to commit transaction: {}",
                        e
                    )))
                })?;
                Ok(ratings)
            }
            Err(e) => {
                // Rollback the transaction on any error
                let _ = txn.rollback().await; // Ignore rollback errors
                Err(e)
            }
        }
    }

    /// Internal method that performs the rating update within a transaction.
    ///
    /// `time_control` selects the K-factor from the module-level table when
    /// known; `None` falls back to `config.k_factor` (legacy behavior).
    pub async fn update_ratings_in_transaction(
        txn: &DatabaseTransaction,
        game_id: Uuid,
        config: &RatingConfig,
        time_control: Option<TimeControlCategory>,
    ) -> Result<(i32, i32), ApiError> {
        // 1. Fetch the game with result
        let game_model = game::Entity::find_by_id(game_id)
            .one(txn)
            .await
            .map_err(|e| {
                ApiError::DatabaseError(DbErr::Custom(format!("Failed to fetch game: {}", e)))
            })?
            .ok_or_else(|| ApiError::NotFound("Game not found".to_string()))?;

        // 2. Check if game is completed
        let game_result = game_model
            .result
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
                return Err(ApiError::BadRequest(
                    "Ratings not updated for abandoned games".to_string(),
                ));
            }
        };

        // 4. Fetch both players with their current ratings
        let white_player = player::Entity::find_by_id(game_model.white_player)
            .one(txn)
            .await
            .map_err(|e| {
                ApiError::DatabaseError(DbErr::Custom(format!(
                    "Failed to fetch white player: {}",
                    e
                )))
            })?
            .ok_or_else(|| ApiError::NotFound("White player not found".to_string()))?;

        let black_player = player::Entity::find_by_id(game_model.black_player)
            .one(txn)
            .await
            .map_err(|e| {
                ApiError::DatabaseError(DbErr::Custom(format!(
                    "Failed to fetch black player: {}",
                    e
                )))
            })?
            .ok_or_else(|| ApiError::NotFound("Black player not found".to_string()))?;

        // 5. Calculate new ratings based on game outcome
        let (new_white_rating, new_black_rating) = Self::calculate_rating_changes(
            white_player.elo_rating,
            black_player.elo_rating,
            white_outcome,
            config,
            time_control,
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
        white_active_model.update(txn).await.map_err(|e| {
            ApiError::DatabaseError(DbErr::Custom(format!(
                "Failed to update white player rating: {}",
                e
            )))
        })?;

        black_active_model.update(txn).await.map_err(|e| {
            ApiError::DatabaseError(DbErr::Custom(format!(
                "Failed to update black player rating: {}",
                e
            )))
        })?;

        Ok((new_white_rating, new_black_rating))
    }

    /// Calculates new ratings based on game outcome using Elo formula.
    ///
    /// The K-factor comes from the time-control table when `time_control` is
    /// known, otherwise from `config.k_factor`.
    fn calculate_rating_changes(
        white_rating: i32,
        black_rating: i32,
        white_outcome: GameOutcome,
        config: &RatingConfig,
        time_control: Option<TimeControlCategory>,
    ) -> (i32, i32) {
        let k_factor = config.effective_k_factor(time_control);
        let (new_white, new_black) = match white_outcome {
            GameOutcome::Win => {
                // White wins, black loses
                calculate_new_ratings(white_rating as u32, black_rating as u32, k_factor)
            }
            GameOutcome::Loss => {
                // White loses, black wins
                let (new_black, new_white) =
                    calculate_new_ratings(black_rating as u32, white_rating as u32, k_factor);
                (new_white, new_black)
            }
            GameOutcome::Draw => {
                // Draw: both players get half points
                Self::calculate_draw_ratings(white_rating, black_rating, k_factor)
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
            .map_err(|e| {
                ApiError::DatabaseError(DbErr::Custom(format!("Failed to fetch player: {}", e)))
            })?
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

        active_model.update(db).await.map_err(|e| {
            ApiError::DatabaseError(DbErr::Custom(format!(
                "Failed to update player rating: {}",
                e
            )))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_rating_changes_white_wins() {
        let config = RatingConfig::default();
        let (new_white, new_black) =
            RatingService::calculate_rating_changes(1500, 1500, GameOutcome::Win, &config, None);

        // Equal ratings, white wins: white gains ~16, black loses ~16
        assert!(new_white > 1500);
        assert!(new_black < 1500);
        assert_eq!(new_white - 1500, 1500 - new_black); // Zero-sum
    }

    #[test]
    fn test_calculate_rating_changes_draw() {
        let config = RatingConfig::default();
        let (new_white, new_black) =
            RatingService::calculate_rating_changes(1600, 1400, GameOutcome::Draw, &config, None);

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

        let (new_white, new_black) =
            RatingService::calculate_rating_changes(50, 2500, GameOutcome::Win, &config, None);

        // Ratings should be clamped to bounds
        assert!(new_white >= config.min_rating);
        assert!(new_white <= config.max_rating);
        assert!(new_black >= config.min_rating);
        assert!(new_black <= config.max_rating);
    }

    #[test]
    fn test_upset_victory_large_rating_change() {
        let config = RatingConfig::default();
        let (new_white, new_black) =
            RatingService::calculate_rating_changes(1200, 1800, GameOutcome::Win, &config, None);

        // Lower rated player beating higher rated should gain significant points
        let white_gain = new_white - 1200;
        let black_loss = 1800 - new_black;

        assert!(white_gain > 20); // Significant gain for upset
        assert!(black_loss > 20); // Significant loss for upset
        assert_eq!(white_gain, black_loss); // Zero-sum
    }

    #[test]
    fn test_k_factor_table_values() {
        assert_eq!(
            RatingConfig::k_factor_for_category(TimeControlCategory::Bullet),
            K_FACTOR_BULLET
        );
        assert_eq!(
            RatingConfig::k_factor_for_category(TimeControlCategory::Blitz),
            K_FACTOR_BLITZ
        );
        assert_eq!(
            RatingConfig::k_factor_for_category(TimeControlCategory::Rapid),
            K_FACTOR_RAPID
        );
        assert_eq!(
            RatingConfig::k_factor_for_category(TimeControlCategory::Classical),
            K_FACTOR_CLASSICAL
        );
        // Classical keeps the historical single K-factor.
        assert_eq!(
            RatingConfig::k_factor_for_category(TimeControlCategory::Classical),
            RatingConfig::default().k_factor
        );
        // Faster controls use strictly larger K-factors.
        assert!(K_FACTOR_BULLET > K_FACTOR_BLITZ);
        assert!(K_FACTOR_BLITZ > K_FACTOR_RAPID);
        assert!(K_FACTOR_RAPID > K_FACTOR_CLASSICAL);
    }

    #[test]
    fn test_bullet_and_classical_use_different_k_factors() {
        let config = RatingConfig::default();
        let (bullet_white, bullet_black) = RatingService::calculate_rating_changes(
            1500,
            1500,
            GameOutcome::Win,
            &config,
            Some(TimeControlCategory::Bullet),
        );
        let (classical_white, classical_black) = RatingService::calculate_rating_changes(
            1500,
            1500,
            GameOutcome::Win,
            &config,
            Some(TimeControlCategory::Classical),
        );

        // Equal ratings: delta is exactly K/2 (56/2 = 28 vs 32/2 = 16).
        assert_eq!((bullet_white, bullet_black), (1528, 1472));
        assert_eq!((classical_white, classical_black), (1516, 1484));
    }

    #[test]
    fn test_rapid_draw_uses_table_k_factor() {
        let config = RatingConfig::default();
        let (rapid_white, rapid_black) = RatingService::calculate_rating_changes(
            1600,
            1400,
            GameOutcome::Draw,
            &config,
            Some(TimeControlCategory::Rapid),
        );
        let (classical_white, classical_black) = RatingService::calculate_rating_changes(
            1600,
            1400,
            GameOutcome::Draw,
            &config,
            Some(TimeControlCategory::Classical),
        );

        // Expected white score ~0.76: rapid (K=40) moves 10 points,
        // classical (K=32) moves 8 points.
        assert_eq!((rapid_white, rapid_black), (1590, 1410));
        assert_eq!((classical_white, classical_black), (1592, 1408));
    }

    #[test]
    fn test_missing_time_control_falls_back_to_config_k_factor() {
        let config = RatingConfig {
            k_factor: 24,
            min_rating: 100,
            max_rating: 3000,
        };
        let (new_white, new_black) =
            RatingService::calculate_rating_changes(1500, 1500, GameOutcome::Win, &config, None);

        // Legacy behavior: delta is exactly k/2 = 12.
        assert_eq!((new_white, new_black), (1512, 1488));
        assert_eq!(config.effective_k_factor(None), 24);
        assert_eq!(
            config.effective_k_factor(Some(TimeControlCategory::Blitz)),
            K_FACTOR_BLITZ
        );
    }
}
