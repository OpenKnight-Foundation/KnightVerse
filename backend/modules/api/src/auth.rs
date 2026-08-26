use actix_web::{
    cookie::{time::Duration, Cookie},
    post, web, HttpRequest, HttpResponse,
};
use std::env;
use tracing::{error, warn};
use uuid::Uuid;
use validator::Validate;

use db::DbPool;
use db_entity::player;
use dto::auth::{
    AuthResponse, ErrorResponse, LoginRequest, LogoutResponse, RefreshResponse,
    RefreshTokenRequest, RegisterRequest,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use security::{JwtService, TokenService, TokenServiceError};
use service::helper::password;

/// Register a new user
#[utoipa::path(
    post,
    path = "/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 400, description = "Validation error", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/register")]
pub async fn register(
    _pool: web::Data<DbPool>,
    payload: web::Json<RegisterRequest>,
) -> HttpResponse {
    if let Err(errors) = payload.validate() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: format!("Validation failed: {:?}", errors),
            code: "VALIDATION_ERROR".to_string(),
        });
    }

    // For now, return a mock response
    increment_auth_events("register", true);
    
    HttpResponse::Created().json(AuthResponse {
        access_token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string(),
        refresh_token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token_expires_in: 604800,
        user_id: 1,
        username: payload.username.clone(),
    })
}

/// Login with credentials — player lookup goes to replica; token write goes to primary.
#[utoipa::path(
    post,
    path = "/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = AuthResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/login")]
pub async fn login(
    pool: web::Data<DbPool>,
    payload: web::Json<LoginRequest>,
    jwt_service: web::Data<JwtService>,
) -> HttpResponse {
    if let Err(errors) = payload.validate() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: format!("Validation failed: {:?}", errors),
            code: "VALIDATION_ERROR".to_string(),
        });
    }

    let username = payload.username.clone();

    // READ: look up player on replica
    let player = match player::Entity::find()
        .filter(player::Column::Username.eq(&username))
        .one(pool.replica())
        .await
    {
        Ok(Some(p)) => p,
        _ => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid username or password".to_string(),
                code: "INVALID_CREDENTIALS".to_string(),
            });
        }
    };

    let stored_hash = String::from_utf8_lossy(&player.password_hash);
    if password::verify_password(&payload.password, &stored_hash).is_err() {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Invalid username or password".to_string(),
            code: "INVALID_CREDENTIALS".to_string(),
        });
    }

    let player_id = player.id;
    let user_id = (player_id.as_u128() & 0x7F_FF_FF_FF) as i32;

    let access_token = match jwt_service.generate_token(user_id, &username, player_id) {
        Ok(t) => t,
        Err(_) => {
            increment_auth_events("login", false);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to generate access token".to_string(),
                code: "TOKEN_ERROR".to_string(),
            });
        }
    };

    let family_id = Uuid::new_v4();
    let refresh_ttl = env::var("REFRESH_TOKEN_TTL_DAYS")
        .unwrap_or_else(|_| "7".to_string())
        .parse::<i64>()
        .unwrap_or(7);

    // WRITE: create refresh token on primary
    let refresh_token =
        match TokenService::generate_refresh_token(pool.primary(), user_id, family_id, refresh_ttl)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to generate refresh token: {}", e);
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "Failed to generate refresh token".to_string(),
                    code: "TOKEN_ERROR".to_string(),
                });
            }
        };

    let mut response = HttpResponse::Ok().json(AuthResponse {
        access_token,
        refresh_token: refresh_token.clone(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token_expires_in: (refresh_ttl * 86400) as usize,
        user_id,
        username,
    });

    let cookie = Cookie::build("refresh_token", refresh_token)
        .http_only(true)
        .secure(false)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(Duration::seconds(refresh_ttl as i64 * 86400))
        .finish();

    response.add_cookie(&cookie).ok();
    
    // Track successful login
    increment_auth_events("login", true);
    
    response
}

/// Refresh tokens — token verification reads from replica; new token write to primary.
#[utoipa::path(
    post,
    path = "/v1/auth/refresh",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Token refresh successful", body = RefreshResponse),
        (status = 401, description = "Invalid or reused refresh token", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/refresh")]
pub async fn refresh(
    pool: web::Data<DbPool>,
    req: HttpRequest,
    payload: Option<web::Json<RefreshTokenRequest>>,
    jwt_service: web::Data<JwtService>,
) -> HttpResponse {
    let refresh_token = if let Some(cookie) = req.cookie("refresh_token") {
        cookie.value().to_string()
    } else if let Some(body) = payload {
        body.refresh_token.clone()
    } else {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Refresh token missing".to_string(),
            code: "MISSING_REFRESH_TOKEN".to_string(),
        });
    };

    let auth_header = match req.headers().get("Authorization") {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                return HttpResponse::Unauthorized().json(ErrorResponse {
                    message: "Invalid authorization header".to_string(),
                    code: "INVALID_AUTH_HEADER".to_string(),
                });
            }
        },
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Missing authorization header".to_string(),
                code: "MISSING_AUTH_HEADER".to_string(),
            });
        }
    };

    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t,
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid authorization format".to_string(),
                code: "INVALID_AUTH_FORMAT".to_string(),
            });
        }
    };

    let claims = match jwt_service.validate_token(token) {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid or expired access token".to_string(),
                code: "INVALID_ACCESS_TOKEN".to_string(),
            });
        }
    };

    // WRITE: mark refresh token as used (must go to primary for atomicity)
    let family_id = match TokenService::verify_and_mark_used(
        pool.primary(),
        &refresh_token,
        claims.user_id,
    )
    .await
    {
        Ok(fid) => fid,
        Err(TokenServiceError::TokenReuseDetected) => {
            warn!("Token reuse detected for player {}", claims.user_id);
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Token reuse detected. Account locked for security.".to_string(),
                code: "TOKEN_THEFT_DETECTED".to_string(),
            });
        }
        Err(TokenServiceError::TokenExpired) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Refresh token has expired".to_string(),
                code: "TOKEN_EXPIRED".to_string(),
            });
        }
        Err(_) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid refresh token".to_string(),
                code: "INVALID_REFRESH_TOKEN".to_string(),
            });
        }
    };

    let new_access_token =
        match jwt_service.generate_token(claims.user_id, &claims.username, claims.player_id) {
            Ok(t) => t,
            Err(_) => {
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    message: "Failed to generate new access token".to_string(),
                    code: "TOKEN_ERROR".to_string(),
                });
            }
        };

    let refresh_ttl = env::var("REFRESH_TOKEN_TTL_DAYS")
        .unwrap_or_else(|_| "7".to_string())
        .parse::<i64>()
        .unwrap_or(7);

    // WRITE: generate new refresh token on primary
    let new_refresh_token = match TokenService::generate_refresh_token(
        pool.primary(),
        claims.user_id,
        family_id,
        refresh_ttl,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to generate new refresh token: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to generate new refresh token".to_string(),
                code: "TOKEN_ERROR".to_string(),
            });
        }
    };

    let mut response = HttpResponse::Ok().json(RefreshResponse {
        access_token: new_access_token,
        refresh_token: new_refresh_token.clone(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
    });

    let cookie = Cookie::build("refresh_token", new_refresh_token)
        .http_only(true)
        .secure(false)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(Duration::seconds(refresh_ttl as i64 * 86400))
        .finish();

    response.add_cookie(&cookie).ok();
    response
}

/// Logout — revoke all tokens on primary.
#[utoipa::path(
    post,
    path = "/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful", body = LogoutResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/logout")]
pub async fn logout(
    pool: web::Data<DbPool>,
    req: HttpRequest,
    jwt_service: web::Data<JwtService>,
) -> HttpResponse {
    let auth_header = match req.headers().get("Authorization") {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                return HttpResponse::Unauthorized().json(ErrorResponse {
                    message: "Invalid authorization header".to_string(),
                    code: "INVALID_AUTH_HEADER".to_string(),
                });
            }
        },
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Missing authorization header".to_string(),
                code: "MISSING_AUTH_HEADER".to_string(),
            });
        }
    };

    let token = match auth_header.strip_prefix("Bearer ") {
        Some(t) => t,
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid authorization format".to_string(),
                code: "INVALID_AUTH_FORMAT".to_string(),
            });
        }
    };

    let claims = match jwt_service.validate_token(token) {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid or expired access token".to_string(),
                code: "INVALID_ACCESS_TOKEN".to_string(),
            });
        }
    };

    let user_id = claims.user_id;

    // WRITE: revoke tokens on primary
    if let Err(e) = TokenService::revoke_player_tokens(pool.primary(), user_id).await {
        error!("Failed to revoke tokens: {}", e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Failed to logout".to_string(),
            code: "LOGOUT_ERROR".to_string(),
        });
    }

    let mut response = HttpResponse::Ok().json(LogoutResponse {
        message: "Logged out successfully".to_string(),
    });

    let cookie = Cookie::build("refresh_token", "")
        .http_only(true)
        .secure(false)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(Duration::seconds(0))
        .finish();

    response.add_cookie(&cookie).ok();
    response
}
