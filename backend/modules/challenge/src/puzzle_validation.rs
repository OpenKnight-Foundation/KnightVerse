use chrono::{DateTime, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use security::jwt::JwtService;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Error types for puzzle validation
#[derive(Error, Debug)]
pub enum PuzzleValidationError {
    #[error("Invalid puzzle format: {0}")]
    InvalidFormat(String),
    #[error("Invalid FEN: {0}")]
    InvalidFen(String),
    #[error("Invalid move: {0}")]
    InvalidMove(String),
    #[error("Puzzle not found")]
    PuzzleNotFound,
    #[error("Incorrect solution")]
    IncorrectSolution,
    #[error("Move validation failed: {0}")]
    MoveValidationFailed(String),
    #[error("Token generation failed")]
    TokenGenerationFailed,
}

/// Puzzle data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Puzzle {
    pub id: Uuid,
    pub fen: String,
    pub title: String,
    pub difficulty: PuzzleDifficulty,
    pub description: String,
    pub solution: Vec<ChessMove>,
    pub hint: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Puzzle difficulty levels
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PuzzleDifficulty {
    Easy,
    Medium,
    Hard,
}

/// Chess move representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChessMove {
    pub from: String,
    pub to: String,
    pub promotion: Option<String>,
}

/// Puzzle solution submission
#[derive(Debug, Deserialize)]
pub struct PuzzleSubmission {
    pub puzzle_id: Uuid,
    pub moves: Vec<ChessMove>,
    pub user_id: i32,
    pub username: String,
}

/// Puzzle validation result
#[derive(Debug, Serialize)]
pub struct PuzzleValidationResult {
    pub success: bool,
    pub correct: bool,
    pub message: String,
    pub reward_token: Option<String>,
    pub reward_amount: Option<f64>,
}

/// Puzzle reward token
#[derive(Debug, Serialize, Deserialize)]
pub struct PuzzleRewardToken {
    pub puzzle_id: Uuid,
    pub user_id: i32,
    pub username: String,
    pub completed_at: DateTime<Utc>,
    pub reward_amount: f64,
    pub token_id: String,
}

/// Puzzle validation service
pub struct PuzzleValidationService {
    jwt_service: JwtService,
    puzzles: Vec<Puzzle>,
}

impl PuzzleValidationService {
    /// Create a new puzzle validation service
    pub fn new(jwt_secret: String) -> Self {
        let jwt_service = JwtService::new(jwt_secret, 3600);
        let puzzles = Self::create_default_puzzles();

        Self {
            jwt_service,
            puzzles,
        }
    }

    /// Create default puzzle set
    fn create_default_puzzles() -> Vec<Puzzle> {
        vec![
            Puzzle {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap(),
                fen: "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4"
                    .to_string(),
                title: "Fork Attack".to_string(),
                difficulty: PuzzleDifficulty::Easy,
                description: "Find the knight fork that wins material".to_string(),
                solution: vec![ChessMove {
                    from: "f3".to_string(),
                    to: "g5".to_string(),
                    promotion: None,
                }],
                hint: Some("Look for a knight move that attacks two pieces".to_string()),
                created_at: Utc::now(),
            },
            Puzzle {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440002").unwrap(),
                fen: "rnbqkb1r/pppp1ppp/5n2/2B1p3/4P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 0 4"
                    .to_string(),
                title: "Pin and Win".to_string(),
                difficulty: PuzzleDifficulty::Medium,
                description: "Use a pin to create a winning advantage".to_string(),
                solution: vec![ChessMove {
                    from: "c4".to_string(),
                    to: "f7".to_string(),
                    promotion: None,
                }],
                hint: Some("The bishop can pin the knight to the king".to_string()),
                created_at: Utc::now(),
            },
            Puzzle {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440003").unwrap(),
                fen: "r1bqk2r/pppp1ppp/2n2n2/2B1p3/4P3/3N1N2/PPPP1PPP/R1BQK2R w KQkq - 0 6"
                    .to_string(),
                title: "Discovered Attack".to_string(),
                difficulty: PuzzleDifficulty::Hard,
                description: "Execute a discovered attack for checkmate".to_string(),
                solution: vec![
                    ChessMove {
                        from: "d3".to_string(),
                        to: "e5".to_string(),
                        promotion: None,
                    },
                    ChessMove {
                        from: "c4".to_string(),
                        to: "f7".to_string(),
                        promotion: None,
                    },
                ],
                hint: Some("Move the knight first to reveal the bishop's attack".to_string()),
                created_at: Utc::now(),
            },
            Puzzle {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440004").unwrap(),
                fen: "rnbqkbnr/pp1ppppp/2p5/3p4/3PP3/2N5/PP1PPPPP/R1BQKBNR w KQkq - 0 3"
                    .to_string(),
                title: "Center Control".to_string(),
                difficulty: PuzzleDifficulty::Easy,
                description: "Control the center with your knight".to_string(),
                solution: vec![ChessMove {
                    from: "c3".to_string(),
                    to: "d5".to_string(),
                    promotion: None,
                }],
                hint: Some("Knights are excellent in the center".to_string()),
                created_at: Utc::now(),
            },
            Puzzle {
                id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440005").unwrap(),
                fen: "rnbqk2r/pppp1ppp/5n2/2B1p3/3PP3/5N2/PPP2PPP/RNBQK2R w KQkq - 0 5".to_string(),
                title: "Double Attack".to_string(),
                difficulty: PuzzleDifficulty::Medium,
                description: "Create a double attack with your bishop".to_string(),
                solution: vec![ChessMove {
                    from: "c4".to_string(),
                    to: "e6".to_string(),
                    promotion: None,
                }],
                hint: Some("Look for squares that attack multiple pieces".to_string()),
                created_at: Utc::now(),
            },
        ]
    }

    /// Get all available puzzles
    pub fn get_puzzles(&self) -> &Vec<Puzzle> {
        &self.puzzles
    }

    /// Get puzzle by ID
    pub fn get_puzzle_by_id(&self, puzzle_id: &Uuid) -> Result<&Puzzle, PuzzleValidationError> {
        self.puzzles
            .iter()
            .find(|p| &p.id == puzzle_id)
            .ok_or(PuzzleValidationError::PuzzleNotFound)
    }

    /// Validate puzzle submission
    pub fn validate_puzzle_solution(
        &self,
        submission: PuzzleSubmission,
    ) -> Result<PuzzleValidationResult, PuzzleValidationError> {
        // Find the puzzle
        let puzzle = self.get_puzzle_by_id(&submission.puzzle_id)?;

        // Validate the solution
        let is_correct = self.validate_solution_moves(puzzle, &submission.moves)?;

        if is_correct {
            // Generate reward token
            let reward_token = self.generate_reward_token(puzzle, &submission)?;

            Ok(PuzzleValidationResult {
                success: true,
                correct: true,
                message: "Puzzle solved correctly! Reward token generated.".to_string(),
                reward_token: Some(reward_token),
                reward_amount: Some(0.01), // 0.01 XLM reward
            })
        } else {
            Ok(PuzzleValidationResult {
                success: true,
                correct: false,
                message: "Incorrect solution. Please try again.".to_string(),
                reward_token: None,
                reward_amount: None,
            })
        }
    }

    /// Validate solution moves using basic move comparison
    fn validate_solution_moves(
        &self,
        puzzle: &Puzzle,
        submitted_moves: &[ChessMove],
    ) -> Result<bool, PuzzleValidationError> {
        // Check if the number of moves matches
        if submitted_moves.len() != puzzle.solution.len() {
            return Ok(false);
        }

        // Validate each move in sequence
        for (submitted_move, expected_move) in submitted_moves.iter().zip(puzzle.solution.iter()) {
            // Compare moves directly
            if !self.moves_equivalent_simple(submitted_move, expected_move) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Check if two moves are equivalent (simple comparison)
    fn moves_equivalent_simple(&self, move1: &ChessMove, move2: &ChessMove) -> bool {
        move1.from == move2.from && move1.to == move2.to && move1.promotion == move2.promotion
    }

    /// Generate reward token for completed puzzle
    fn generate_reward_token(
        &self,
        puzzle: &Puzzle,
        submission: &PuzzleSubmission,
    ) -> Result<String, PuzzleValidationError> {
        let reward_data = PuzzleRewardToken {
            puzzle_id: puzzle.id,
            user_id: submission.user_id,
            username: submission.username.clone(),
            completed_at: Utc::now(),
            reward_amount: 0.01,
            token_id: Uuid::new_v4().to_string(),
        };

        let token_claims = serde_json::to_value(reward_data)
            .map_err(|_| PuzzleValidationError::TokenGenerationFailed)?;

        let token = encode(
            &Header::default(),
            &token_claims,
            &EncodingKey::from_secret(self.get_jwt_secret().as_ref()),
        )
        .map_err(|_| PuzzleValidationError::TokenGenerationFailed)?;

        Ok(token)
    }

    /// Get JWT secret key
    fn get_jwt_secret(&self) -> &str {
        &self.jwt_service.secret_key
    }

    /// Verify reward token
    pub fn verify_reward_token(
        &self,
        token: &str,
    ) -> Result<PuzzleRewardToken, PuzzleValidationError> {
        let token_data = jsonwebtoken::decode::<serde_json::Value>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(self.get_jwt_secret().as_ref()),
            &jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256),
        )
        .map_err(|_| PuzzleValidationError::InvalidFormat("Invalid token".to_string()))?;

        let reward_token: PuzzleRewardToken =
            serde_json::from_value(token_data.claims).map_err(|_| {
                PuzzleValidationError::InvalidFormat("Invalid token format".to_string())
            })?;

        Ok(reward_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_service() -> PuzzleValidationService {
        PuzzleValidationService::new("test_secret_key".to_string())
    }

    #[test]
    fn test_get_puzzles() {
        let service = create_test_service();
        assert_eq!(service.get_puzzles().len(), 5);
    }

    #[test]
    fn test_get_puzzle_by_id() {
        let service = create_test_service();
        let puzzle_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();
        let puzzle = service.get_puzzle_by_id(&puzzle_id).unwrap();
        assert_eq!(puzzle.title, "Fork Attack");
    }

    #[test]
    fn test_validate_correct_solution() {
        let service = create_test_service();
        let puzzle_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();

        let submission = PuzzleSubmission {
            puzzle_id,
            moves: vec![ChessMove {
                from: "f3".to_string(),
                to: "g5".to_string(),
                promotion: None,
            }],
            user_id: 1,
            username: "testuser".to_string(),
        };

        let result = service.validate_puzzle_solution(submission).unwrap();
        assert!(result.correct);
        assert!(result.reward_token.is_some());
        assert_eq!(result.reward_amount, Some(0.01));
    }

    #[test]
    fn test_validate_incorrect_solution() {
        let service = create_test_service();
        let puzzle_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();

        let submission = PuzzleSubmission {
            puzzle_id,
            moves: vec![ChessMove {
                from: "f3".to_string(),
                to: "f4".to_string(),
                promotion: None,
            }],
            user_id: 1,
            username: "testuser".to_string(),
        };

        let result = service.validate_puzzle_solution(submission).unwrap();
        assert!(!result.correct);
        assert!(result.reward_token.is_none());
    }
}
