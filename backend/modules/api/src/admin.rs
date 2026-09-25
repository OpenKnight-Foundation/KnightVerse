//! Admin-only audit log endpoints (BE-91).
//!
//! `Claims` has no administrator flag today, so admins are configured
//! out-of-band via the `ADMIN_USER_IDS` (comma-separated `user_id`s) and
//! `ADMIN_USERNAMES` (comma-separated usernames) environment variables. When
//! neither is set, no caller is an administrator and every route here returns
//! `403` — a secure default.

use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use db::DbPool;
use error::error::ApiError;
use security::jwt::Claims;
use serde::Deserialize;
use serde_json::{json, Value};
use service::admin_audit::{
    list_admin_actions, perform_admin_action, AdminActionFilter, AdminActionType,
};
use uuid::Uuid;

/// Whether the authenticated caller is an administrator.
pub fn is_admin(claims: &Claims) -> bool {
    let by_id = std::env::var("ADMIN_USER_IDS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .filter_map(|part| part.trim().parse::<i32>().ok())
                .any(|id| id == claims.user_id)
        })
        .unwrap_or(false);
    if by_id {
        return true;
    }

    std::env::var("ADMIN_USERNAMES")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .any(|name| !name.is_empty() && name == claims.username)
        })
        .unwrap_or(false)
}

/// Extract the caller's claims, or produce the appropriate rejection response.
fn require_admin(req: &HttpRequest) -> Result<Claims, HttpResponse> {
    match req.extensions().get::<Claims>() {
        Some(claims) if is_admin(claims) => Ok(claims.clone()),
        Some(_) => Err(HttpResponse::Forbidden().json(json!({
            "error": "Administrator privileges required",
            "code": 403
        }))),
        None => Err(HttpResponse::Unauthorized().json(json!({
            "error": "Authentication required",
            "code": 401
        }))),
    }
}

/// Query parameters for the audit-log listing.
#[derive(Debug, Deserialize)]
pub struct ListActionsQuery {
    pub actor_id: Option<Uuid>,
    pub target_id: Option<Uuid>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

/// Request body for recording a moderator action.
#[derive(Debug, Deserialize)]
pub struct RecordActionRequest {
    /// One of `ban`, `unban`, `mute`, `unmute`, `resolve_dispute`, `flag_cheating`.
    pub action: String,
    pub target_id: Uuid,
    pub reason: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

// ---------------------------------------------------------------------------
// GET /v1/admin/actions — admin-only, paginated, filterable (READ → replica)
// ---------------------------------------------------------------------------
#[get("/actions")]
pub async fn list_actions(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    query: web::Query<ListActionsQuery>,
) -> HttpResponse {
    if let Err(rejection) = require_admin(&req) {
        return rejection;
    }

    let filter = AdminActionFilter {
        actor_id: query.actor_id,
        target_id: query.target_id,
        page: query.page,
        per_page: query.per_page,
    };

    match list_admin_actions(pool.get_ref(), filter).await {
        Ok(page) => HttpResponse::Ok().json(json!({
            "message": "Admin actions",
            "data": page
        })),
        Err(err) => err.error_response(),
    }
}

// ---------------------------------------------------------------------------
// POST /v1/admin/actions — admin-only, records one moderator action (WRITE → primary)
// ---------------------------------------------------------------------------
#[post("/actions")]
pub async fn record_action(
    req: HttpRequest,
    pool: web::Data<DbPool>,
    payload: web::Json<RecordActionRequest>,
) -> HttpResponse {
    let claims = match require_admin(&req) {
        Ok(claims) => claims,
        Err(rejection) => return rejection,
    };

    let action = match AdminActionType::parse(&payload.action) {
        Some(action) => action,
        None => {
            return ApiError::BadRequest(format!("Unknown admin action: {}", payload.action))
                .error_response()
        }
    };

    let metadata = if payload.metadata.is_null() {
        json!({})
    } else {
        payload.metadata.clone()
    };

    match perform_admin_action(
        pool.get_ref(),
        claims.player_id,
        action,
        payload.target_id,
        payload.reason.clone(),
        metadata,
    )
    .await
    {
        Ok(stored) => HttpResponse::Created().json(json!({
            "message": "Admin action recorded",
            "data": stored
        })),
        Err(err) => err.error_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use security::jwt::{Claims, TokenType};

    fn claims_for(user_id: i32, username: &str) -> Claims {
        Claims {
            sub: user_id.to_string(),
            user_id,
            player_id: Uuid::new_v4(),
            username: username.to_string(),
            exp: 0,
            iat: 0,
            jti: None,
            token_type: TokenType::Access,
        }
    }

    #[test]
    fn only_allowlisted_callers_are_admins() {
        std::env::remove_var("ADMIN_USERNAMES");
        std::env::set_var("ADMIN_USER_IDS", "42, 7");

        assert!(is_admin(&claims_for(42, "moderator")));
        assert!(is_admin(&claims_for(7, "other")));
        assert!(!is_admin(&claims_for(99, "regular")));

        std::env::remove_var("ADMIN_USER_IDS");
    }
}
