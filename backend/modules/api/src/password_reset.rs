// BE-159: password reset flow. Both endpoints are mounted inside the auth
// scope, which already applies the governor and Redis rate limiters.
use actix_web::{post, web, HttpResponse};
use chrono::{Duration, Utc};
use db::DbPool;
use db_entity::{password_reset_token, player};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use security::token_service::TokenService;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const RESET_TTL_MINUTES: i64 = 30;

#[derive(Debug, Deserialize)]
pub struct ForgotRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct ForgotResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct ResetResponse {
    pub message: String,
}

#[utoipa::path(
    post,
    path = "/v1/auth/password/forgot",
    request_body = ForgotRequest,
    responses((status = 200, description = "If the account exists, a reset link is issued")),
    tag = "Authentication"
)]
#[post("/password/forgot")]
pub async fn forgot_password(
    pool: web::Data<DbPool>,
    payload: web::Json<ForgotRequest>,
) -> HttpResponse {
    // Always 200: never reveal whether an email is registered.
    let ok = HttpResponse::Ok().json(ForgotResponse {
        message: "If the account exists, a reset link has been sent".to_string(),
    });

    let account = match player::Entity::find()
        .filter(player::Column::Email.eq(&payload.email))
        .one(pool.primary())
        .await
    {
        Ok(Some(account)) => account,
        _ => return ok,
    };

    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let now = Utc::now();
    let active = password_reset_token::ActiveModel {
        id: Set(Uuid::new_v4()),
        player_id: Set(account.id),
        token_hash: Set(TokenService::hash_token(&token)),
        expires_at: Set(now + Duration::minutes(RESET_TTL_MINUTES)),
        used_at: Set(None),
        created_at: Set(now),
    };
    if active.insert(pool.primary()).await.is_err() {
        return ok;
    }

    let base = std::env::var("PASSWORD_RESET_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    tracing::info!("Password reset link for {}: {}/reset-password?token={}", account.email, base, token);

    ok
}

#[utoipa::path(
    post,
    path = "/v1/auth/password/reset",
    request_body = ResetRequest,
    responses(
        (status = 200, description = "Password reset", body = ResetResponse),
        (status = 400, description = "Invalid, expired or reused token")
    ),
    tag = "Authentication"
)]
#[post("/password/reset")]
pub async fn reset_password(
    pool: web::Data<DbPool>,
    payload: web::Json<ResetRequest>,
) -> HttpResponse {
    if payload.new_password.len() < 8 || payload.new_password.len() > 64 {
        return HttpResponse::BadRequest().json(ResetResponse {
            message: "Password must be between 8 and 64 characters".to_string(),
        });
    }

    let hash = TokenService::hash_token(&payload.token);
    let record = match password_reset_token::Entity::find()
        .filter(password_reset_token::Column::TokenHash.eq(&hash))
        .one(pool.primary())
        .await
    {
        Ok(Some(record)) => record,
        _ => {
            return HttpResponse::BadRequest().json(ResetResponse {
                message: "Invalid or expired reset token".to_string(),
            })
        }
    };

    if record.used_at.is_some() || record.expires_at < Utc::now() {
        return HttpResponse::BadRequest().json(ResetResponse {
            message: "Invalid or expired reset token".to_string(),
        });
    }

    let hash = match service::helper::password::hash_password(&payload.new_password) {
        Ok(hash) => hash.into_bytes(),
        Err(_) => {
            return HttpResponse::InternalServerError().json(ResetResponse {
                message: "Failed to hash password".to_string(),
            })
        }
    };

    let account = match player::Entity::find_by_id(record.player_id)
        .one(pool.primary())
        .await
    {
        Ok(Some(account)) => account,
        _ => {
            return HttpResponse::BadRequest().json(ResetResponse {
                message: "Invalid or expired reset token".to_string(),
            })
        }
    };

    let mut active: player::ActiveModel = account.into();
    active.password_hash = Set(hash);
    if active.update(pool.primary()).await.is_err() {
        return HttpResponse::InternalServerError().json(ResetResponse {
            message: "Failed to update password".to_string(),
        });
    }

    // Revoke every refresh-token family for this player.
    let user_id = ((record.player_id.as_u128() & 0x7F_FF_FF_FF) as u64) as i32;
    let _ = TokenService::revoke_player_tokens(pool.primary(), user_id).await;

    let mut token_active: password_reset_token::ActiveModel = record.into();
    token_active.used_at = Set(Some(Utc::now()));
    let _ = token_active.update(pool.primary()).await;

    HttpResponse::Ok().json(ResetResponse {
        message: "Password has been reset".to_string(),
    })
}

