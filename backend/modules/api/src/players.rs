use actix_web::{
    delete, get, post, put,
    web::{Json, Path},
    HttpResponse,
};
use dto::players::{DisplayPlayer, NewPlayer, UpdatePlayer, UpdatedPlayer};
use error::error::ApiError;
use serde_json::json;
use validator::Validate;

use service::players::{
    add_player as add_new_player, delete_player as delete_player_by_id,
    find_player_by_id as get_single_player_by_id, update_player as update_player_by_id,
};
use uuid::Uuid;

#[utoipa::path(
    post,
    path = "/v1/players",
    responses(
        (status = 200, description = "New player added", body=PlayerAdded),
        (status = 400, description = "Bad request", body=InvalidCredentialsResponse)
    )
)]
#[post("")]
pub async fn add_player(payload: Json<NewPlayer>) -> HttpResponse {
    match payload.0.validate() {
        Ok(_) => {
            let player = add_new_player(payload.0).await;

            match player {
                Ok(plyr) => HttpResponse::Ok().json(json!({
                    "message":"New player added",
                    "data":DisplayPlayer::from(plyr)
                })),
                Err(err) => err.error_response(),
            }
        }
        Err(errors) => ApiError::ValidationError(errors).error_response(),
    }
}

#[utoipa::path(
    get,
    path = "/v1/players/{id}",
    params(
        ("id" = String, Path, description = "Player ID in UUID format", format="uuid")
    ),
    responses(
        (status = 200, description = "Player found", body=PlayerFound),
        (status = 404, description = "Not found", body=NotFoundResponse)
    )
)]
#[get("/{id}")]
pub async fn find_player_by_id(id: Path<Uuid>) -> HttpResponse {
    let player = get_single_player_by_id(id.into_inner()).await;

    match player {
        Ok(plyr) => HttpResponse::Ok().json(json!({
            "message":"Player found",
            "data":{
                "player": DisplayPlayer::from(plyr)
            }
        })),
        Err(err) => err.error_response(),
    }
}

#[utoipa::path(
    put,
    path = "/v1/players/{id}",
    params(
        ("id" = String, Path, description = "Player ID in UUID format", format="uuid")
    ),
    responses(
        (status = 200, description = "Player updated", body=PlayerUpdated),
        (status = 404, description = "Not found", body=NotFoundResponse)
    )
)]
#[put("/{id}")]
pub async fn update_player(id: Path<Uuid>, payload: Json<UpdatePlayer>) -> HttpResponse {
    match payload.0.validate() {
        Ok(_) => {
            let player = update_player_by_id(id.into_inner(), payload.0).await;

            match player {
                Ok(plyr) => HttpResponse::Ok().json(json!({
                    "message":"Player updated",
                    "data":{
                        "player": UpdatedPlayer::from(plyr)
                    }
                })),
                Err(err) => err.error_response(),
            }
        }
        Err(err) => ApiError::ValidationError(err).error_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/v1/players/{id}",
    params(
        ("id" = String, Path, description = "Player ID in UUID format", format="uuid")
    ),
    responses(
        (status = 200, description = "Player deleted", body=PlayerDeleted),
        (status = 404, description = "Not found", body=NotFoundResponse)
    )
)]
#[delete("/{id}")]
pub async fn delete_player(id: Path<Uuid>) -> HttpResponse {
    match delete_player_by_id(id.into_inner()).await {
        Ok(_) => HttpResponse::Ok().json(json!({
            "message":"Player deleted",
            "data":{}
        })),
        Err(err) => err.error_response(),
    }
}
