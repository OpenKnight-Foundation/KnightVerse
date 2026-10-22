use actix_web::{
    cookie::{time::Duration, Cookie},
    post, web, HttpRequest, HttpResponse,
};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, warn};
use uuid::Uuid;
use validator::Validate;

use crate::metrics::increment_auth_events;
use db::DbPool;
use db_entity::player;
use dto::auth::{
    AuthResponse, ErrorResponse, LoginRequest, LogoutResponse, RefreshResponse,
    RefreshTokenRequest, RegisterRequest,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use security::{JwtService, TokenService, TokenServiceError};
use service::helper::password;

/// WebSocket tickets are intentionally short-lived and single-use. The opaque
/// value itself is the Redis key suffix; the associated claims never reach the
/// browser.
pub const WS_TICKET_TTL_SECONDS: usize = 60;

#[utoipa::path(
    post,
    path = "/v1/auth/ws-ticket",
    responses(
        (status = 200, description = "WebSocket ticket issued"),
        (status = 401, description = "Missing or invalid access token")
    ),
    tag = "Authentication"
)]
#[post("/ws-ticket")]
pub async fn issue_ws_ticket(
    req: HttpRequest,
    jwt_service: web::Data<JwtService>,
    redis_pool: web::Data<Pool>,
) -> HttpResponse {
    let token = match req
        .headers()
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
    {
        Some(token) if !token.is_empty() => token,
        _ => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Missing or invalid authorization token"
            }));
        }
    };

    let claims = match jwt_service.validate_token(token) {
        Ok(claims) if claims.token_type == security::jwt::TokenType::Access => claims,
        _ => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "message": "Invalid or expired access token"
            }));
        }
    };

    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(error) => {
            error!("Failed to acquire Redis connection for WebSocket ticket: {}", error);
            return HttpResponse::ServiceUnavailable().finish();
        }
    };

    if let Some(jti) = claims.jti.as_deref() {
        let blacklisted: redis::RedisResult<bool> =
            conn.exists(format!("token_blacklist:{}", jti)).await;
        match blacklisted {
            Ok(true) => {
                return HttpResponse::Unauthorized().json(serde_json::json!({
                    "message": "Access token has been revoked"
                }));
            }
            Err(error) => {
                error!("Failed to check access-token revocation for WebSocket ticket: {}", error);
                return HttpResponse::ServiceUnavailable().finish();
            }
            Ok(false) => {}
        }
    }

    let claims_json = match serde_json::to_string(&claims) {
        Ok(json) => json,
        Err(error) => {
            error!("Failed to encode WebSocket ticket claims: {}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // Retry a rare UUID collision rather than overwriting an outstanding ticket.
    for _ in 0..3 {
        let ticket = Uuid::new_v4().to_string();
        let key = format!("ws_ticket:{}", ticket);
        let stored: redis::RedisResult<Option<String>> = redis::cmd("SET")
            .arg(&key)
            .arg(&claims_json)
            .arg("EX")
            .arg(WS_TICKET_TTL_SECONDS)
            .arg("NX")
            .query_async(&mut conn)
            .await;
        match stored {
            Ok(Some(_)) => {
                return HttpResponse::Ok()
                    .insert_header(("Cache-Control", "no-store"))
                    .json(serde_json::json!({
                        "ticket": ticket,
                        "expires_in": WS_TICKET_TTL_SECONDS
                    }));
            }
            Ok(None) => continue,
            Err(error) => {
                error!("Failed to store WebSocket ticket in Redis: {}", error);
                return HttpResponse::ServiceUnavailable().finish();
            }
        }
    }

    HttpResponse::ServiceUnavailable().finish()
}

async fn add_jti_to_blacklist(
    redis_pool: &Pool,
    jti: &str,
    ttl_seconds: usize,
) -> Result<(), String> {
    if jti.trim().is_empty() {
        return Ok(());
    }

    let mut conn = redis_pool.get().await.map_err(|e| e.to_string())?;
    let key = format!("token_blacklist:{}", jti);
    let ttl = ttl_seconds.max(1) as usize;
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg("1")
        .arg("EX")
        .arg(ttl)
        .query_async(&mut conn)
        .await
        .map_err(|e| format!("Redis blacklist write failed: {}", e))?;
    Ok(())
}

async fn is_jti_blacklisted(redis_pool: &Pool, jti: &str) -> bool {
    if jti.trim().is_empty() {
        return false;
    }

    let mut conn = match redis_pool.get().await {
        Ok(c) => c,
        Err(_) => return false,
    };
    let key = format!("token_blacklist:{}", jti);
    conn.exists(&key).await.unwrap_or(false)
}

/// Register a new user
#[utoipa::path(
    post,
    path = "/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = AuthResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 409, description = "Username or email already taken", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/register")]
pub async fn register(
    pool: web::Data<DbPool>,
    payload: web::Json<RegisterRequest>,
    jwt_service: web::Data<JwtService>,
) -> HttpResponse {
    if let Err(errors) = payload.validate() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: format!("Validation failed: {:?}", errors),
            code: "VALIDATION_ERROR".to_string(),
        });
    }

    let username = payload.username.clone();
    let email = payload.email.clone();

    // Reject taken username or email up front (unique keys exist on both).
    let existing = player::Entity::find()
        .filter(
            player::Column::Username
                .eq(&username)
                .or(player::Column::Email.eq(&email)),
        )
        .one(pool.replica())
        .await;
    if let Ok(Some(_)) = existing {
        return HttpResponse::Conflict().json(ErrorResponse {
            message: "Username or email is already taken".to_string(),
            code: "USER_EXISTS".to_string(),
        });
    }

    let password_hash = match password::hash_password(&payload.password) {
        Ok(h) => h.into_bytes(),
        Err(e) => {
            error!("Failed to hash password during registration: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to create account".to_string(),
                code: "INTERNAL_ERROR".to_string(),
            });
        }
    };

    let player_id = Uuid::new_v4();
    let new_player = player::ActiveModel {
        id: Set(player_id),
        username: Set(username.clone()),
        email: Set(email.clone()),
        password_hash: Set(password_hash),
        biography: Set(String::new()),
        country: Set(String::new()),
        flair: Set(String::new()),
        real_name: Set(username.clone()),
        is_enabled: Set(true),
        ..Default::default()
    };

    if let Err(e) = player::Entity::insert(new_player).exec(pool.primary()).await {
        // Unique index violations (racing registrations) map to 409; anything else is a server fault.
        let detail = e.to_string();
        if detail.contains("duplicate") || detail.contains("unique") {
            return HttpResponse::Conflict().json(ErrorResponse {
                message: "Username or email is already taken".to_string(),
                code: "USER_EXISTS".to_string(),
            });
        }
        error!("Registration insert failed: {}", e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Failed to create account".to_string(),
            code: "INTERNAL_ERROR".to_string(),
        });
    }

    // Same identity and token flow as login from here on.
    let user_id = (player_id.as_u128() & 0x7F_FF_FF_FF) as i32;

    let access_token = match jwt_service.generate_token(user_id, &username, player_id) {
        Ok(t) => t,
        Err(_) => {
            increment_auth_events("register", false);
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

    let mut response = HttpResponse::Created().json(AuthResponse {
        access_token,
        refresh_token: refresh_token.clone(),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token_expires_in: (refresh_ttl * 86400) as usize,
        user_id,
        username,
        wallet_address: None,
    });

    let cookie = Cookie::build("refresh_token", refresh_token)
        .http_only(true)
        .secure(false)
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(Duration::seconds(refresh_ttl as i64 * 86400))
        .finish();

    response.add_cookie(&cookie).ok();

    increment_auth_events("register", true);

    response
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
        wallet_address: player.wallet_address.clone(),
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
    redis_pool: web::Data<Pool>,
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

    if let Some(jti) = claims.jti.as_ref() {
        if is_jti_blacklisted(redis_pool.as_ref(), jti).await {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Token revoked".to_string(),
                code: "TOKEN_REVOKED".to_string(),
            });
        }
    }

    // WRITE: mark refresh token as used (must go to primary for atomicity)
    let family_id =
        match TokenService::verify_and_mark_used(pool.primary(), &refresh_token, claims.user_id)
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
    redis_pool: web::Data<Pool>,
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

    if let Some(jti) = claims.jti.as_ref() {
        let ttl = claims.exp.saturating_sub(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as usize,
        );
        if let Err(e) = add_jti_to_blacklist(redis_pool.as_ref(), jti, ttl).await {
            error!("Failed to blacklist access token jti {}: {}", jti, e);
        }
    }

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

/// Logout all sessions for the current user and immediately blacklist the active token.
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout_all",
    responses(
        (status = 200, description = "Logout all sessions successful", body = LogoutResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/logout_all")]
pub async fn logout_all(
    pool: web::Data<DbPool>,
    req: HttpRequest,
    jwt_service: web::Data<JwtService>,
    redis_pool: web::Data<Pool>,
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

    if let Some(jti) = claims.jti.as_ref() {
        let ttl = claims.exp.saturating_sub(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as usize,
        );
        if let Err(e) = add_jti_to_blacklist(redis_pool.as_ref(), jti, ttl).await {
            error!("Failed to blacklist access token jti {}: {}", jti, e);
        }
    }

    if let Err(e) = TokenService::revoke_player_tokens(pool.primary(), claims.user_id).await {
        error!(
            "Failed to revoke all sessions for user {}: {}",
            claims.user_id, e
        );
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Failed to logout all devices".to_string(),
            code: "LOGOUT_ALL_ERROR".to_string(),
        });
    }

    let mut response = HttpResponse::Ok().json(LogoutResponse {
        message: "Logged out all devices successfully".to_string(),
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
