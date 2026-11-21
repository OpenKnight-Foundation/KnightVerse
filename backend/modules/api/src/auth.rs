use actix_web::{
    cookie::{time::Duration, Cookie},
    post, web, HttpRequest, HttpResponse,
};
use chrono::Utc;
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};
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

/// Email-verification links are valid for 24 hours (BE-160).
const EMAIL_VERIFICATION_TTL_SECONDS: usize = 24 * 60 * 60;

fn email_verification_key(token: &str) -> String {
    format!("email_verification:{token}")
}

/// Store a fresh single-use verification token in Redis and emit the link.
/// Delivery is logged rather than handed to an external mailer so the flow
/// remains testable without SMTP credentials; a configured
/// `EMAIL_VERIFICATION_BASE_URL` controls the host shown in the link.
async fn issue_email_verification(
    redis_pool: &Pool,
    player_id: Uuid,
    email: &str,
) -> Result<String, String> {
    let token = Uuid::new_v4().to_string();
    let mut conn = redis_pool
        .get()
        .await
        .map_err(|e| format!("Redis connection failed: {e}"))?;
    let key = email_verification_key(&token);
    let stored: redis::RedisResult<Option<String>> = redis::cmd("SET")
        .arg(&key)
        .arg(player_id.to_string())
        .arg("EX")
        .arg(EMAIL_VERIFICATION_TTL_SECONDS)
        .arg("NX")
        .query_async(&mut conn)
        .await;
    match stored {
        Ok(Some(_)) => {}
        Ok(None) => return Err("verification token collision".to_string()),
        Err(e) => return Err(format!("Redis SET failed: {e}")),
    }

    let base = env::var("EMAIL_VERIFICATION_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    info!(
        "Email verification link for {}: {}/verify-email?token={}",
        email, base, token
    );
    Ok(token)
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct VerificationResponse {
    pub message: String,
}

#[utoipa::path(
    post,
    path = "/v1/auth/verify-email",
    request_body = VerifyEmailRequest,
    responses(
        (status = 200, description = "Email verified", body = VerificationResponse),
        (status = 400, description = "Invalid or expired token", body = ErrorResponse)
    ),
    tag = "Authentication"
)]
#[post("/verify-email")]
pub async fn verify_email(
    pool: web::Data<DbPool>,
    redis_pool: web::Data<Pool>,
    payload: web::Json<VerifyEmailRequest>,
) -> HttpResponse {
    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to acquire Redis connection for email verification: {e}");
            return HttpResponse::ServiceUnavailable().finish();
        }
    };
    let key = email_verification_key(&payload.token);
    let player_id: Option<String> = match conn.get(&key).await {
        Ok(value) => value,
        Err(e) => {
            error!("Failed to read email verification token: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };
    let Some(player_id_str) = player_id else {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Invalid or expired verification token".to_string(),
            code: "INVALID_VERIFICATION_TOKEN".to_string(),
        });
    };
    let Ok(player_id) = Uuid::parse_str(&player_id_str) else {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: "Invalid or expired verification token".to_string(),
            code: "INVALID_VERIFICATION_TOKEN".to_string(),
        });
    };

    let model = match player::Entity::find_by_id(player_id).one(pool.primary()).await {
        Ok(Some(model)) => model,
        Ok(None) => {
            return HttpResponse::BadRequest().json(ErrorResponse {
                message: "Invalid or expired verification token".to_string(),
                code: "INVALID_VERIFICATION_TOKEN".to_string(),
            });
        }
        Err(e) => {
            error!("Failed to load player during email verification: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut active: player::ActiveModel = model.into();
    active.email_verified_at = Set(Some(Utc::now().into()));
    if let Err(e) = active.update(pool.primary()).await {
        error!("Failed to persist email verification: {e}");
        return HttpResponse::InternalServerError().finish();
    }

    let _: redis::RedisResult<()> = conn.del(&key).await;
    increment_auth_events("verify_email", true);

    HttpResponse::Ok().json(VerificationResponse {
        message: "Email verified successfully".to_string(),
    })
}

#[utoipa::path(
    post,
    path = "/v1/auth/resend-verification",
    responses(
        (status = 200, description = "Verification link reissued", body = VerificationResponse),
        (status = 401, description = "Missing or invalid access token")
    ),
    tag = "Authentication"
)]
#[post("/resend-verification")]
pub async fn resend_verification(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    redis_pool: web::Data<Pool>,
) -> HttpResponse {
    let claims = match req.extensions().get::<security::jwt::Claims>() {
        Some(claims) => claims.clone(),
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Missing or invalid authorization token".to_string(),
                code: "UNAUTHORIZED".to_string(),
            });
        }
    };

    let model = match player::Entity::find_by_id(claims.player_id)
        .one(pool.replica())
        .await
    {
        Ok(Some(model)) => model,
        Ok(None) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Account not found".to_string(),
                code: "UNAUTHORIZED".to_string(),
            });
        }
        Err(e) => {
            error!("Failed to load player during verification resend: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    if model.email_verified_at.is_some() {
        return HttpResponse::Ok().json(VerificationResponse {
            message: "Email is already verified".to_string(),
        });
    }

    match issue_email_verification(redis_pool.get_ref(), model.id, &model.email).await {
        Ok(_) => HttpResponse::Ok().json(VerificationResponse {
            message: "Verification link sent".to_string(),
        }),
        Err(e) => {
            error!("Failed to issue verification token: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

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
    redis_pool: web::Data<Pool>,
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
        email_verified_at: Set(None),
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

    // BE-160: issue a single-use verification link. Delivery failing must not
    // roll back an otherwise valid registration; the user can resend later.
    if let Err(e) = issue_email_verification(redis_pool.get_ref(), player_id, &email).await {
        warn!("Failed to issue email verification token for {}: {e}", email);
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
    path = "/v1/auth/logout_all",
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
