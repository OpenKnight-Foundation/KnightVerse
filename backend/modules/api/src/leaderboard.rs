// BE-158: cached global leaderboard endpoint.
use actix_web::{get, web, HttpResponse};
use deadpool_redis::Pool;
use redis::AsyncCommands;
use serde::Deserialize;

use db::DbPool;

#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default = "default_limit")]
    pub limit: u64,
    pub cursor: Option<String>,
}

fn default_category() -> String {
    "blitz".to_string()
}

fn default_limit() -> u64 {
    50
}

const CACHE_TTL_SECONDS: usize = 60;

fn cache_key(category: &str, limit: u64, cursor: Option<&str>) -> String {
    format!(
        "leaderboard:{}:{}:{}",
        category,
        limit,
        cursor.unwrap_or("head")
    )
}

#[utoipa::path(
    get,
    path = "/v1/leaderboard",
    params(
        ("category" = Option<String>, Query, description = "Time control category"),
        ("limit" = Option<u64>, Query, description = "Page size (1-100)"),
        ("cursor" = Option<String>, Query, description = "Pagination cursor")
    ),
    responses(
        (status = 200, description = "Ranked leaderboard page"),
        (status = 400, description = "Invalid cursor")
    ),
    tag = "Leaderboard"
)]
#[get("")]
pub async fn get_leaderboard(
    query: web::Query<LeaderboardQuery>,
    pool: web::Data<DbPool>,
    redis_pool: web::Data<Pool>,
) -> HttpResponse {
    let key = cache_key(&query.category, query.limit, query.cursor.as_deref());

    let mut conn = match redis_pool.get().await {
        Ok(conn) => conn,
        Err(_) => return HttpResponse::ServiceUnavailable().finish(),
    };

    let cached: Option<String> = conn.get(&key).await.ok();
    if let Some(cached) = cached {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&cached) {
            return HttpResponse::Ok().json(value);
        }
    }

    let response = match service::leaderboard::get_leaderboard(
        pool.get_ref(),
        &query.category,
        query.limit,
        query.cursor.as_deref(),
    )
    .await
    {
        Ok(response) => response,
        Err(e) => return e.error_response(),
    };

    if let Ok(json) = serde_json::to_string(&response) {
        let _: redis::RedisResult<()> = redis::cmd("SET")
            .arg(&key)
            .arg(json)
            .arg("EX")
            .arg(CACHE_TTL_SECONDS)
            .query_async(&mut conn)
            .await;
    }

    HttpResponse::Ok().json(response)
}
