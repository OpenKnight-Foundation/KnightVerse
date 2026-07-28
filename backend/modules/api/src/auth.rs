use actix_web::{web, HttpResponse, HttpRequest, post, cookie::{Cookie, time::Duration}};
use validator::Validate;
use std::env;
use uuid::Uuid;

use dto::auth::{RegisterRequest, LoginRequest, AuthResponse, ErrorResponse, RefreshTokenRequest, RefreshResponse, LogoutResponse};
use security::{JwtService, TokenService, TokenServiceError};
use sea_orm::DatabaseConnection;
use crate::metrics::increment_auth_events;

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
    _db: web::Data<DatabaseConnection>,
    payload: web::Json<RegisterRequest>,
) -> HttpResponse {
    // Validate input
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

/// Login with credentials
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
    db: web::Data<DatabaseConnection>,
    payload: web::Json<LoginRequest>,
    jwt_service: web::Data<JwtService>,
) -> HttpResponse {
    // Validate input
    if let Err(errors) = payload.validate() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            message: format!("Validation failed: {:?}", errors),
            code: "VALIDATION_ERROR".to_string(),
        });
    }

    // For MVP: mock user with ID 1
    let user_id = 1;
    let username = payload.username.clone();

    // Generate access token
    let access_token = match jwt_service.generate_token(user_id, &username) {
        Ok(t) => t,
        Err(_) => {
            increment_auth_events("login", false);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to generate access token".to_string(),
                code: "TOKEN_ERROR".to_string(),
            });
        }
    };

    // Generate refresh token
    let family_id = Uuid::new_v4();
    let refresh_ttl = env::var("REFRESH_TOKEN_TTL_DAYS")
        .unwrap_or_else(|_| "7".to_string())
        .parse::<i64>()
        .unwrap_or(7);

    let refresh_token = match TokenService::generate_refresh_token(&db, user_id, family_id, refresh_ttl).await {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to generate refresh token: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to generate refresh token".to_string(),
                code: "TOKEN_ERROR".to_string(),
            });
        }
    };

    // Build response with cookie
    let mut response = HttpResponse::Ok()
        .json(AuthResponse {
            access_token,
            refresh_token: refresh_token.clone(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token_expires_in: (refresh_ttl * 86400) as usize,
            user_id,
            username,
        });

    // Set HTTP-only secure cookie
    let cookie = Cookie::build("refresh_token", refresh_token)
        .http_only(true)
        .secure(false) // Set to true in production HTTPS
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(Duration::seconds(refresh_ttl as i64 * 86400))
        .finish();

    response.add_cookie(&cookie).ok();
    
    // Track successful login
    increment_auth_events("login", true);
    
    response
}

/// Refresh tokens - rotate refresh token and get new access token
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
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
    payload: Option<web::Json<RefreshTokenRequest>>,
    jwt_service: web::Data<JwtService>,
) -> HttpResponse {
    // Extract refresh token from cookie or request body
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

    // Extract user from access token in Authorization header
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

    // Extract Bearer token
    let token = if auth_header.starts_with("Bearer ") {
        &auth_header[7..]
    } else {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Invalid authorization format".to_string(),
            code: "INVALID_AUTH_FORMAT".to_string(),
        });
    };

    // Validate access token and get user info
    let claims = match jwt_service.validate_token(token) {
        Ok(c) => c,
        Err(_) => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                message: "Invalid or expired access token".to_string(),
                code: "INVALID_ACCESS_TOKEN".to_string(),
            });
        }
    };

    // Verify refresh token and mark as used
    let family_id = match TokenService::verify_and_mark_used(&db, &refresh_token, claims.user_id).await {
        Ok(fid) => fid,
        Err(TokenServiceError::TokenReuseDetected) => {
            log::warn!("Token reuse detected for player {}", claims.user_id);
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

    // Generate new access token
    let new_access_token = match jwt_service.generate_token(claims.user_id, &claims.username) {
        Ok(t) => t,
        Err(_) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to generate new access token".to_string(),
                code: "TOKEN_ERROR".to_string(),
            });
        }
    };

    // Generate new refresh token in same family
    let refresh_ttl = env::var("REFRESH_TOKEN_TTL_DAYS")
        .unwrap_or_else(|_| "7".to_string())
        .parse::<i64>()
        .unwrap_or(7);

    let new_refresh_token = match TokenService::generate_refresh_token(&db, claims.user_id, family_id, refresh_ttl).await {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to generate new refresh token: {}", e);
            return HttpResponse::InternalServerError().json(ErrorResponse {
                message: "Failed to generate new refresh token".to_string(),
                code: "TOKEN_ERROR".to_string(),
            });
        }
    };

    // Build response with new cookie
    let mut response = HttpResponse::Ok()
        .json(RefreshResponse {
            access_token: new_access_token,
            refresh_token: new_refresh_token.clone(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
        });

    // Set new HTTP-only cookie
    let cookie = Cookie::build("refresh_token", new_refresh_token)
        .http_only(true)
        .secure(false) // Set to true in production HTTPS
        .same_site(actix_web::cookie::SameSite::Strict)
        .max_age(Duration::seconds(refresh_ttl as i64 * 86400))
        .finish();

    response.add_cookie(&cookie).ok();
    response
}

/// Logout - revoke all tokens
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
    db: web::Data<DatabaseConnection>,
    req: HttpRequest,
) -> HttpResponse {
    // Extract user from access token
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

    let token = if auth_header.starts_with("Bearer ") {
        &auth_header[7..]
    } else {
        return HttpResponse::Unauthorized().json(ErrorResponse {
            message: "Invalid authorization format".to_string(),
            code: "INVALID_AUTH_FORMAT".to_string(),
        });
    };

    // We would validate token here, but for now just extract user_id from the request
    // In a real implementation, we'd use a JWT service to validate and extract claims
    // For MVP, we'll accept it and revoke for user_id 1
    let user_id = 1;

    // Revoke all tokens for this player
    if let Err(e) = TokenService::revoke_player_tokens(&db, user_id).await {
        log::error!("Failed to revoke tokens: {}", e);
        return HttpResponse::InternalServerError().json(ErrorResponse {
            message: "Failed to logout".to_string(),
            code: "LOGOUT_ERROR".to_string(),
        });
    }

    // Clear the refresh token cookie
    let mut response = HttpResponse::Ok()
        .json(LogoutResponse {
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
