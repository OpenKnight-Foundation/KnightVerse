use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

use crate::players::{DisplayPlayer, UpdatedPlayer};

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct PlayerAddedBody {
    pub player: DisplayPlayer,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct DeletedBody {}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct PlayerUpdatedBody {
    pub player: UpdatedPlayer,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct PlayerAdded {
    #[schema(example = "New player added")]
    pub message: String,
    pub body: PlayerAddedBody,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct PlayerFound {
    #[schema(example = "Player found")]
    pub message: String,
    pub body: PlayerAddedBody,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct PlayerUpdated {
    #[schema(example = "Player updated")]
    pub message: String,
    pub body: PlayerUpdatedBody,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct PlayerDeleted {
    #[schema(example = "Player deleted")]
    pub message: String,
    pub body: DeletedBody,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct InvalidCredentialsResponse {
    #[schema(example = "Invalid credentials")]
    pub error: String,
    #[schema(example = 400)]
    pub code: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct NotFoundResponse {
    #[schema(example = "Player not found")]
    pub error: String,
    #[schema(example = 404)]
    pub code: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Validate)]
pub struct ValidationErrorResponse {
    #[schema(example = "Invalid input data")]
    pub error: String,
    #[schema(example = 400)]
    pub code: i32,
    #[schema(example = json!(["FEN string is invalid", "Depth must be between 1 and 20"]))]
    pub details: Option<Vec<String>>,
}
