use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct RegisterRequest {
    #[validate(length(
        min = 3,
        max = 32,
        message = "Username must be between 3 and 32 characters"
    ))]
    #[schema(example = "chess_master")]
    pub username: String,

    #[validate(email(message = "Invalid email format"))]
    #[schema(example = "user@example.com")]
    pub email: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    #[schema(example = "SecurePass123!")]
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct LoginRequest {
    #[validate(length(min = 3, message = "Username must be at least 3 characters"))]
    #[schema(example = "chess_master")]
    pub username: String,

    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    #[schema(example = "SecurePass123!")]
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AuthResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: String,

    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub refresh_token: String,

    #[schema(example = "Bearer")]
    pub token_type: String,

    #[schema(example = 3600)]
    pub expires_in: usize,

    #[schema(example = 604800)]
    pub refresh_token_expires_in: usize,

    #[schema(example = "1")]
    pub user_id: i32,

    #[schema(example = "chess_master")]
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RefreshTokenRequest {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub refresh_token: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    #[schema(example = "Invalid credentials")]
    pub message: String,

    #[schema(example = "401")]
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserInfo {
    #[schema(value_type = String, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: Uuid,

    #[schema(example = "chess_master")]
    pub username: String,

    #[schema(example = "chess@example.com")]
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TokenResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: String,

    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub refresh_token: String,

    #[schema(example = "Bearer")]
    pub token_type: String,

    #[schema(example = 3600)]
    pub expires_in: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct RefreshResponse {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: String,

    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub refresh_token: String,

    #[schema(example = "Bearer")]
    pub token_type: String,

    #[schema(example = 3600)]
    pub expires_in: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LogoutResponse {
    #[schema(example = "Logged out successfully")]
    pub message: String,
}

pub type LoginResponse = AuthResponse;
