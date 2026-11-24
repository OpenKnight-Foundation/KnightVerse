//! Global leaderboard (BE-158): active, enabled players ranked by rating.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use db::DbPool;
use db_entity::player;
use error::error::ApiError;
use sea_orm::{ColumnTrait, EntityTrait, Order, QueryFilter, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u64,
    pub player_id: uuid::Uuid,
    pub username: String,
    pub elo_rating: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardResponse {
    pub category: String,
    pub entries: Vec<LeaderboardEntry>,
    pub next_cursor: Option<String>,
}

fn encode_offset(offset: u64) -> String {
    URL_SAFE_NO_PAD.encode(offset.to_string())
}

fn decode_offset(cursor: &str) -> Result<u64, String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| "invalid cursor".to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "invalid cursor".to_string())?;
    text.parse::<u64>().map_err(|_| "invalid cursor".to_string())
}

pub async fn get_leaderboard(
    pool: &DbPool,
    category: &str,
    limit: u64,
    cursor: Option<&str>,
) -> Result<LeaderboardResponse, ApiError> {
    let offset = match cursor {
        Some(cursor) => decode_offset(cursor).map_err(ApiError::BadRequest)?,
        None => 0,
    };
    let limit = limit.clamp(1, 100);

    let mut rows = player::Entity::find()
        .filter(player::Column::IsEnabled.eq(true))
        .order_by(player::Column::EloRating, Order::Desc)
        .order_by(player::Column::Id, Order::Asc)
        .limit(limit + 1)
        .offset(offset)
        .all(pool.replica())
        .await?;

    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);

    let entries = rows
        .into_iter()
        .enumerate()
        .map(|(idx, row)| LeaderboardEntry {
            rank: offset + idx as u64 + 1,
            player_id: row.id,
            username: row.username,
            elo_rating: row.elo_rating,
        })
        .collect();

    Ok(LeaderboardResponse {
        category: category.to_string(),
        entries,
        next_cursor: if has_more {
            Some(encode_offset(offset + limit))
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_roundtrip() {
        let encoded = encode_offset(42);
        assert_eq!(decode_offset(&encoded).unwrap(), 42);
        assert!(decode_offset("!!!").is_err());
    }
}
