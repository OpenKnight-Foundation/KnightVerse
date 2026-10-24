use actix_web::{web, HttpResponse};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

const MAX_POINTS: i64 = 1000;

/// Parses `?range=`. Outer None = invalid. Inner None = "all time".
pub fn parse_range(r: Option<&str>) -> Option<Option<Duration>> {
    match r.unwrap_or("30d") {
        "7d" => Some(Some(Duration::days(7))),
        "30d" => Some(Some(Duration::days(30))),
        "90d" => Some(Some(Duration::days(90))),
        "1y" => Some(Some(Duration::days(365))),
        "all" => Some(None),
        _ => None,
    }
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub range: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
pub struct RatingPoint {
    pub game_id: Uuid,
    pub ended_at: DateTime<Utc>,
    pub rating: i32,
}

pub async fn online_players(/* REPLACE: the same app data extractor other ws code uses */) -> HttpResponse {
    let online: usize = 0; // REPLACE with: distinct player count from LobbyState (see step 4)
    HttpResponse::Ok().json(serde_json::json!({ "online": online }))
}

pub async fn rating_history(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<HistoryQuery>,
) -> HttpResponse {
    let player_id = path.into_inner();
    let Some(range) = parse_range(query.range.as_deref()) else {
        return HttpResponse::BadRequest()
            .json(serde_json::json!({ "error": "range must be one of 7d, 30d, 90d, 1y, all" }));
    };
    let since: Option<DateTime<Utc>> = range.map(|d| Utc::now() - d);

    // REPLACE table/column names with the real ones from the migrations.
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM players WHERE id = $1")
        .bind(player_id)
        .fetch_one(pool.get_ref())
        .await;
    match exists {
        Ok(0) => return HttpResponse::NotFound().json(serde_json::json!({ "error": "player not found" })),
        Err(_) => return HttpResponse::InternalServerError().finish(),
        _ => {}
    }

    let rows = sqlx::query_as::<_, RatingPoint>(
        "SELECT id AS game_id, ended_at,
                CASE WHEN white_id = $1 THEN white_rating_after ELSE black_rating_after END AS rating
         FROM games
         WHERE status = 'completed'
           AND (white_id = $1 OR black_id = $1)
           AND ($2::timestamptz IS NULL OR ended_at >= $2)
         ORDER BY ended_at DESC
         LIMIT $3",
    )
    .bind(player_id)
    .bind(since)
    .bind(MAX_POINTS)
    .fetch_all(pool.get_ref())
    .await;

    match rows {
        Ok(mut points) => {
            points.reverse(); // oldest first
            HttpResponse::Ok().json(serde_json::json!({
                "player_id": player_id,
                "range": query.range.clone().unwrap_or_else(|| "30d".into()),
                "points": points,
            }))
        }
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    // "online" MUST be registered before any /players/{id} route.
    cfg.route("/players/online", web::get().to(online_players))
        .route("/players/{id}/rating-history", web::get().to(rating_history));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_30_days() {
        assert_eq!(parse_range(None), Some(Some(Duration::days(30))));
    }

    #[test]
    fn valid_ranges() {
        assert_eq!(parse_range(Some("7d")), Some(Some(Duration::days(7))));
        assert_eq!(parse_range(Some("1y")), Some(Some(Duration::days(365))));
        assert_eq!(parse_range(Some("all")), Some(None));
    }

    #[test]
    fn invalid_range_rejected() {
        assert_eq!(parse_range(Some("5d")), None);
        assert_eq!(parse_range(Some("")), None);
    }
}