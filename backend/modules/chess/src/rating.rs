//! Rating updates with a per-time-control K-factor (Elo) and Glicko-2 support.
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
//!
//! # Glicko-2
//!
//! Each player carries a rating `r`, a rating deviation `RD` (uncertainty) and
//! a volatility `sigma`, tracked per time-control category. Players whose RD is
//! above `PROVISIONAL_RD_THRESHOLD` are reported as provisional. Existing Elo
//! ratings migrate with the default RD and volatility.

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

/// Glicko-2 default rating (Glicko scale).
pub const GLICKO_DEFAULT_RATING: f64 = 1500.0;
/// Glicko-2 default rating deviation for new / migrated players.
pub const GLICKO_DEFAULT_RD: f64 = 350.0;
/// Glicko-2 default volatility.
pub const GLICKO_DEFAULT_VOLATILITY: f64 = 0.06;
/// Rating deviation above which a player is considered provisional.
pub const PROVISIONAL_RD_THRESHOLD: f64 = 110.0;
/// System constant controlling volatility change speed (tau).
pub const GLICKO_TAU: f64 = 0.5;

const GLICKO_SCALE: f64 = 173.7178;
const CONVERGENCE_EPSILON: f64 = 0.000001;

/// Service for handling rating calculations and updates after game completion
pub struct RatingService;

/// Represents the outcome of a game from a player's perspective
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameOutcome {
    Win,
    Loss,
    Draw,
}

/// Glicko-2 rating state for a single player in a single time-control category.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glicko2Rating {
    /// Rating on the Glicko-2 scale (Elo-like, centered on 1500).
    pub rating: f64,
    /// Rating deviation (uncertainty).
    pub rd: f64,
    /// Volatility.
    pub volatility: f64,
}

impl Default for Glicko2Rating {
    fn default() -> Self {
        Self {
            rating: GLICKO_DEFAULT_RATING,
            rd: GLICKO_DEFAULT_RD,
            volatility: GLICKO_DEFAULT_VOLATILITY,
        }
    }
}

impl Glicko2Rating {
    /// Builds a Glicko-2 state from a legacy Elo rating, keeping the default
    /// RD and volatility so migrated players start as provisional.
    pub fn from_elo(elo: i32) -> Self {
        Self {
            rating: elo as f64,
            rd: GLICKO_DEFAULT_RD,
            volatility: GLICKO_DEFAULT_VOLATILITY,
        }
    }

    /// Whether the player's rating is still provisional (RD too high).
    pub fn is_provisional(&self) -> bool {
        self.rd > PROVISIONAL_RD_THRESHOLD
    }
}

/// Glicko-2 match result for one game, from the updating player's perspective.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glicko2Match {
    pub opponent: Glicko2Rating,
    /// Score: 1.0 win, 0.5 draw, 0.0 loss.
    pub score: f64,
}

/// Configuration for rating calculations
#[derive(Debug, Clone)]
pub struct RatingConfig {
    /// K-factor for Elo rating calculations (typically 32 for new players, 16 for experienced)
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

/// Converts a Glicko-2 rating to the internal scale (mu).
fn to_mu(rating: f64) -> f64 {
    (rating - GLICKO_DEFAULT_RATING) / GLICKO_SCALE
}

/// Converts an internal-scale mu back to a Glicko-2 rating.
fn from_mu(mu: f64) -> f64 {
    mu * GLICKO_SCALE + GLICKO_DEFAULT_RATING
}

/// Converts a Glicko-2 RD to the internal scale (phi).
fn to_phi(rd: f64) -> f64 {
    rd / GLICKO_SCALE
}

/// Converts an internal-scale phi back to a Glicko-2 RD.
fn from_phi(phi: f64) -> f64 {
    phi * GLICKO_SCALE
}

fn g(phi: f64) -> f64 {
    1.0 / (1.0 + 3.0 * phi * phi / (std::f64::consts::PI * std::f64::consts::PI)).sqrt()
}

fn expected(mu: f64, mu_j: f64, phi_j: f64) -> f64 {
    1.0 / (1.0 + (-g(phi_j) * (mu - mu_j)).exp())
}

/// Computes the new Glicko-2 state for a player after a rating period
/// containing the given matches (Glickman, 2013).
///
/// An empty `matches` slice only inflates RD for inactivity.
pub fn glicko2_update(player: Glicko2Rating, matches: &[Glicko2Match]) -> Glicko2Rating {
    let mu = to_mu(player.rating);
    let phi = to_phi(player.rd);
    let sigma = player.volatility;

    if matches.is_empty() {
        let phi_star = (phi * phi + sigma * sigma).sqrt();
        return Glicko2Rating {
            rating: player.rating,
            rd: from_phi(phi_star),
            volatility: sigma,
        };
    }

    // Step 3: estimated variance v.
    let mut v_inv = 0.0;
    // Step 4: estimated improvement delta.
    let mut delta_sum = 0.0;
    for m in matches {
        let mu_j = to_mu(m.opponent.rating);
        let phi_j = to_phi(m.opponent.rd);
        let gj = g(phi_j);
        let ej = expected(mu, mu_j, phi_j);
        v_inv += gj * gj * ej * (1.0 - ej);
        delta_sum += gj * (m.score - ej);
    }
    let v = 1.0 / v_inv;
    let delta = v * delta_sum;

    // Step 5: new volatility via Illinois-style root finding.
    let a = (sigma * sigma).ln();
    let f = |x: f64| {
        let ex = x.exp();
        let num = ex * (delta * delta - phi * phi - v - ex);
        let den = 2.0 * (phi * phi + v + ex).powi(2);
        num / den - (x - a) / (GLICKO_TAU * GLICKO_TAU)
    };

    let mut big_a = a;
    let mut big_b = if delta * delta > phi * phi + v {
        (delta * delta - phi * phi - v).ln()
    } else {
        let mut k = 1.0;
        while f(a - k * GLICKO_TAU) < 0.0 {
            k += 1.0;
        }
        a - k * GLICKO_TAU
    };
    let mut f_a = f(big_a);
    let mut f_b = f(big_b);
    while (big_b - big_a).abs() > CONVERGENCE_EPSILON {
        let big_c = big_a + (big_a - big_b) * f_a / (f_b - f_a);
        let f_c = f(big_c);
        if f_c * f_b <= 0.0 {
            big_a = big_b;
            f_a = f_b;
        } else {
            f_a /= 2.0;
        }
        big_b = big_c;
        f_b = f_c;
    }
    let new_sigma = (big_a / 2.0).exp();

    // Step 6: pre-rating-period RD.
    let phi_star = (phi * phi + new_sigma * new_sigma).sqrt();
    // Step 7: new phi and mu.
    let new_phi = 1.0 / (1.0 / (phi_star * phi_star) + 1.0 / v).sqrt();
    let new_mu = mu + new_phi * new_phi * delta_sum;

    Glicko2Rating {
        rating: from_mu(new_mu),
        rd: from_phi(new_phi),
        volatility: new_sigma,
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
    /// * `Ok((white_new_rating, black_new_rating))` - New Elo ratings for both players
    /// * `Err(ApiError)` - If game not found, players not found, or database error
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn glicko2_reference_example_from_paper() {
        // Glickman (2013) worked example: player 1500/200/0.06 vs.
        // (1400, 30), (1550, 100), (1700, 300) with scores 1, 0, 0.
        let player = Glicko2Rating {
            rating: 1500.0,
            rd: 200.0,
            volatility: 0.06,
        };
        let matches = [
            Glicko2Match {
                opponent: Glicko2Rating { rating: 1400.0, rd: 30.0, volatility: 0.06 },
                score: 1.0,
            },
            Glicko2Match {
                opponent: Glicko2Rating { rating: 1550.0, rd: 100.0, volatility: 0.06 },
                score: 0.0,
            },
            Glicko2Match {
                opponent: Glicko2Rating { rating: 1700.0, rd: 300.0, volatility: 0.06 },
                score: 0.0,
            },
        ];
        let updated = glicko2_update(player, &matches);
        assert!(approx(updated.rating, 1464.06, 0.01), "rating {}", updated.rating);
        assert!(approx(updated.rd, 151.52, 0.01), "rd {}", updated.rd);
        assert!(approx(updated.volatility, 0.05999, 0.0001), "vol {}", updated.volatility);
    }

    #[test]
    fn glicko2_inactivity_only_inflates_rd() {
        let player = Glicko2Rating {
            rating: 1500.0,
            rd: 50.0,
            volatility: 0.06,
        };
        let updated = glicko2_update(player, &[]);
        assert_eq!(updated.rating, 1500.0);
        assert_eq!(updated.volatility, 0.06);
        assert!(updated.rd > 50.0);
    }

    #[test]
    fn migration_from_elo_is_provisional() {
        let migrated = Glicko2Rating::from_elo(1800);
        assert_eq!(migrated.rating, 1800.0);
        assert_eq!(migrated.rd, GLICKO_DEFAULT_RD);
        assert_eq!(migrated.volatility, GLICKO_DEFAULT_VOLATILITY);
        assert!(migrated.is_provisional());
    }

    #[test]
    fn established_player_is_not_provisional() {
        let player = Glicko2Rating { rating: 1500.0, rd: 50.0, volatility: 0.06 };
        assert!(!player.is_provisional());
    }

    #[test]
    fn k_factor_table_values() {
        assert_eq!(RatingConfig::k_factor_for_category(TimeControlCategory::Bullet), 56);
        assert_eq!(RatingConfig::k_factor_for_category(TimeControlCategory::Blitz), 48);
        assert_eq!(RatingConfig::k_factor_for_category(TimeControlCategory::Rapid), 40);
        assert_eq!(RatingConfig::k_factor_for_category(TimeControlCategory::Classical), 32);
    }

    #[test]
    fn effective_k_factor_falls_back_when_unknown() {
        let config = RatingConfig::default();
        assert_eq!(config.effective_k_factor(None), config.k_factor);
        assert_eq!(
            config.effective_k_factor(Some(TimeControlCategory::Bullet)),
            K_FACTOR_BULLET
        );
    }
}
