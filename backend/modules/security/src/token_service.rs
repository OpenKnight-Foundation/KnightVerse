use base64::Engine;
use chrono::{Duration, Utc};
use db_entity::refresh_token;
use rand::Rng;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Errors that can occur during token operations
#[derive(Debug)]
pub enum TokenServiceError {
    TokenNotFound,
    TokenReuseDetected,
    TokenInvalid,
    TokenExpired,
    DatabaseError(String),
}

impl fmt::Display for TokenServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenNotFound => write!(f, "Token not found"),
            Self::TokenReuseDetected => write!(f, "Token reuse detected - potential theft"),
            Self::TokenInvalid => write!(f, "Token is invalid or revoked"),
            Self::TokenExpired => write!(f, "Token has expired"),
            Self::DatabaseError(e) => write!(f, "Database error: {}", e),
        }
    }
}

impl From<DbErr> for TokenServiceError {
    fn from(err: DbErr) -> Self {
        Self::DatabaseError(err.to_string())
    }
}

impl std::error::Error for TokenServiceError {}

/// Token service for generating, validating, and managing refresh tokens
#[derive(Clone, Debug)]
pub struct TokenService;

impl TokenService {
    /// Generate a new refresh token
    ///
    /// Returns a tuple of (plaintext_token, token_record)
    pub async fn generate_refresh_token(
        db: &DatabaseConnection,
        player_id: i32,
        family_id: Uuid,
        ttl_days: i64,
    ) -> Result<String, TokenServiceError> {
        // 1. Generate 32 random bytes
        let mut rng = rand::thread_rng();
        let random_bytes: [u8; 32] = rng.gen();

        // 2. Base64 encode for URL safety
        let token = base64::engine::general_purpose::STANDARD.encode(random_bytes);

        // 3. SHA256 hash for storage
        let token_hash = Self::hash_token(&token);

        // 4. Calculate expiration
        let now = Utc::now();
        let expires_at = now + Duration::days(ttl_days);

        // 5. Store in database
        let refresh_token = refresh_token::ActiveModel {
            id: Set(Uuid::new_v4()),
            player_id: Set(player_id),
            family_id: Set(family_id),
            token_hash: Set(token_hash),
            created_at: Set(now),
            used_at: Set(None),
            expires_at: Set(expires_at),
            is_revoked: Set(false),
        };

        refresh_token.insert(db).await?;

        Ok(token)
    }

    /// Verify a refresh token and mark it as used
    ///
    /// Returns the family_id if valid, or an error if theft is detected
    pub async fn verify_and_mark_used(
        db: &DatabaseConnection,
        token: &str,
        player_id: i32,
    ) -> Result<Uuid, TokenServiceError> {
        let token_hash = Self::hash_token(token);

        // 1. Find the token record
        let token_record = refresh_token::Entity::find()
            .filter(refresh_token::Column::TokenHash.eq(&token_hash))
            .filter(refresh_token::Column::PlayerId.eq(player_id))
            .one(db)
            .await?;

        let token_record = token_record.ok_or(TokenServiceError::TokenNotFound)?;

        // 2. Check if already used (THEFT DETECTION!)
        if token_record.used_at.is_some() {
            // Token already used - this is token reuse!
            // Invalidate the entire family as a security measure
            let family_id = token_record.family_id;
            Self::invalidate_token_family(db, family_id).await?;
            return Err(TokenServiceError::TokenReuseDetected);
        }

        // 3. Check if revoked
        if token_record.is_revoked {
            return Err(TokenServiceError::TokenInvalid);
        }

        // 4. Check if expired
        if token_record.expires_at < Utc::now() {
            return Err(TokenServiceError::TokenExpired);
        }

        // 5. Mark as used - update the record directly
        refresh_token::Entity::update_many()
            .col_expr(refresh_token::Column::UsedAt, Expr::value(Utc::now()))
            .filter(refresh_token::Column::Id.eq(token_record.id))
            .exec(db)
            .await?;

        Ok(token_record.family_id)
    }

    /// Invalidate all tokens in a family (used for theft detection)
    pub async fn invalidate_token_family(
        db: &DatabaseConnection,
        family_id: Uuid,
    ) -> Result<(), TokenServiceError> {
        refresh_token::Entity::update_many()
            .col_expr(refresh_token::Column::IsRevoked, Expr::value(true))
            .filter(refresh_token::Column::FamilyId.eq(family_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// Revoke all tokens for a player (used for logout)
    pub async fn revoke_player_tokens(
        db: &DatabaseConnection,
        player_id: i32,
    ) -> Result<(), TokenServiceError> {
        refresh_token::Entity::update_many()
            .col_expr(refresh_token::Column::IsRevoked, Expr::value(true))
            .filter(refresh_token::Column::PlayerId.eq(player_id))
            .exec(db)
            .await?;

        Ok(())
    }

    /// Hash a token using SHA256
    fn hash_token(token: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}
