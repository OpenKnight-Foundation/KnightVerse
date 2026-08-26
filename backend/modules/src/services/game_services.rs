use crate::models::game::*;
use crate::utils::fen_generator::generate_fen;
use crate::db::game_queries::insert_game;
use std::collections::HashMap;
use uuid::Uuid;
use sqlx::PgPool;

pub async fn create_game_service(
    payload: CreateGameRequest,
    db_pool: &PgPool,
) -> Result<CreateGameResponse, String> {
    if payload.players.len() < 1 || payload.players.len() > 2 {
        return Err("Must provide 1 or 2 players".into());
    }

    let variant_str = match payload.variant {
        GameVariant::Standard => "standard",
        GameVariant::Chess960 => "chess960",
        GameVariant::ThreeCheck => "three-check",
        
    };

    let initial_state = generate_fen(variant_str);

    // Assign colors
    let white = &payload.players[0];
    let black = if payload.players.len() == 2 {
        &payload.players[1]
    } else {
        "AI"
    };

    // DB insert
    let game_id = insert_game(db_pool, white, black, variant_str, &initial_state, payload.rated)
        .await
        .map_err(|e| e.to_string())?;

    // Build response
    let mut player_assignments = HashMap::new();
    player_assignments.insert(white.clone(), "white".into());
    player_assignments.insert(black.into(), "black".into());

    let session_token = Uuid::new_v4().to_string();

    Ok(CreateGameResponse {
        game_id: game_id.clone(),
        session_token,
        initial_state,
        player_assignments,
        join_url: format!("https://knightverse.io/games/{}", game_id),
    })
}
