use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::models::{GameStatus, PieceColor, Player, Room, ServerMessage};

const LATENCY_BUFFER_MS: u64 = 750;

type MessageSender = broadcast::Sender<ServerMessage>;

use crate::pubsub::RedisPubSub;

pub struct ServerState {
    pub rooms: HashMap<String, Room>,
    pub message_senders: HashMap<String, MessageSender>,
    pub pubsub: Option<RedisPubSub>,
}

lazy_static::lazy_static! {
    pub static ref GAME_STATE: Arc<Mutex<ServerState>> = Arc::new(Mutex::new(ServerState {
        rooms: HashMap::new(),
        message_senders: HashMap::new(),
        pubsub: None,
    }));
}

// Initialize the game state
pub fn init_game_state() {
    // This function is called at startup to ensure the lazy_static is initialized
    let _guard = GAME_STATE.lock().unwrap();
    log::info!("Game state initialized");
}

// Get a clone of the message sender for a room
pub fn get_room_sender(room_id: &str) -> Option<MessageSender> {
    let state = GAME_STATE.lock().unwrap();
    state.message_senders.get(room_id).cloned()
}

// Create a new room
pub fn create_room() -> String {
    let room_id = Uuid::new_v4().to_string();
    let (tx, _) = broadcast::channel(100);

    let mut state = GAME_STATE.lock().unwrap();
    state.rooms.insert(room_id.clone(), Room::new(room_id.clone()));
    state.message_senders.insert(room_id.clone(), tx);

    room_id
}

// Create a new room with custom time control
pub fn create_room_with_time(initial_time_ms: u64, increment_ms: u64) -> String {
    let room_id = Uuid::new_v4().to_string();
    let (tx, _) = broadcast::channel(100);

    let mut state = GAME_STATE.lock().unwrap();
    state.rooms.insert(
        room_id.clone(),
        Room::new_with_time(room_id.clone(), initial_time_ms, increment_ms),
    );
    state.message_senders.insert(room_id.clone(), tx);

    log::info!(
        "Created room {} with time control: {}ms + {}ms increment",
        room_id, initial_time_ms, increment_ms
    );

    room_id
}

// Join a room
pub fn join_room(room_id: &str, player_id: &str, player_name: Option<String>) -> Result<ServerMessage, String> {
    let mut state = GAME_STATE.lock().unwrap();

    // Check if room exists, create if not
    if !state.rooms.contains_key(room_id) {
        drop(state); // Release the lock before creating the room

        // Create the room with the requested ID
        let (tx, _) = broadcast::channel(100);

        state = GAME_STATE.lock().unwrap();
        // Re-check after re-locking to avoid race condition
        if !state.rooms.contains_key(room_id) {
            state.rooms.insert(room_id.to_string(), Room::new(room_id.to_string()));
            state.message_senders.insert(room_id.to_string(), tx);
        }
    }

    let room = state.rooms.get_mut(room_id).unwrap();

    // Check if this is the second player (game will start)
    let is_game_starting = room.players.len() == 1;

    // Create player
    let player = Player {
        id: player_id.to_string(),
        name: player_name.unwrap_or_else(|| format!("Player {}", player_id)),
        color: None,
    };

    // Add player to room
    room.add_player(player)?;

    // If second player joined, start White's clock
    if is_game_starting {
        let now_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| format!("System clock error: {}", e))?
            .as_millis() as u64;
        room.last_move_at = Some(now_ms);
        log::info!("Game started in room {}, clock started at {}ms", room_id, now_ms);
    }

    // Create response message
    let response = ServerMessage::RoomJoined {
        room_id: room_id.to_string(),
        player_id: player_id.to_string(),
        players: room.players.clone(),
        game_state: room.game_state.clone(),
    };

    // Broadcast to other players in the room
    if let Some(sender) = state.message_senders.get(room_id) {
        if let Err(e) = sender.send(response.clone()) {
            log::warn!("Failed to broadcast RoomJoined message: {:?}", e);
        }
    }

    Ok(response)
}

// Send a move
pub fn send_move(room_id: &str, player_id: &str, move_notation: &str) -> Result<ServerMessage, String> {
    let mut state = GAME_STATE.lock().unwrap();

    // Check if room exists
    let room = state.rooms.get_mut(room_id).ok_or_else(|| "Room not found".to_string())?;

    // Check if player is in the room
    if !room.players.iter().any(|p| p.id == player_id) {
        return Err("Player not in room".to_string());
    }

    // Check if game has started
    let game_state = room.game_state.as_mut().ok_or_else(|| "Game not started".to_string())?;

    let now_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("System clock error: {}", e))?
        .as_millis() as u64;

    // Determine which player is moving based on current turn
    let is_white = matches!(game_state.current_turn, PieceColor::White);
    let player_remaining = if is_white { room.white_remaining_ms } else { room.black_remaining_ms };

    // Calculate elapsed time since last move
    let elapsed_ms = room.last_move_at
        .map(|last| now_ms.saturating_sub(last))
        .unwrap_or(0);

    // Check if move is within time (with latency buffer)
    if elapsed_ms > player_remaining + LATENCY_BUFFER_MS {
        // Time exceeded - reject move and end game
        let winner_color = if is_white { "Black" } else { "White" };
        let loser_color = if is_white { "White" } else { "Black" };

        log::warn!(
            "Move rejected: player {} in room {} exceeded time. Elapsed: {}ms, Remaining: {}ms, Buffer: {}ms",
            player_id, room_id, elapsed_ms, player_remaining, LATENCY_BUFFER_MS
        );

        game_state.status = GameStatus::Timeout;

        // Find winner and loser player IDs
        let (winner_id, loser_id) = room.players.iter().fold(
            (String::new(), String::new()),
            |(winner, loser), p| {
                match &p.color {
                    Some(PieceColor::White) if is_white => (winner, p.id.clone()),
                    Some(PieceColor::White) => (p.id.clone(), loser),
                    Some(PieceColor::Black) if !is_white => (winner, p.id.clone()),
                    Some(PieceColor::Black) => (p.id.clone(), loser),
                    None => (winner, loser),
                }
            }
        );

        // Broadcast timeout
        if let Some(sender) = state.message_senders.get(room_id) {
            let timeout_msg = ServerMessage::GameTimeout {
                room_id: room_id.to_string(),
                winner_id: winner_id.clone(),
                loser_id: loser_id.clone(),
                reason: format!("{} ran out of time", loser_color),
            };
            let _ = sender.send(timeout_msg);
        }

        return Err(format!("Time expired. {} wins on time.", winner_color));
    }

    // Deduct elapsed time from player's clock and add increment
    if is_white {
        room.white_remaining_ms = room.white_remaining_ms.saturating_sub(elapsed_ms);
        room.white_remaining_ms += room.increment_ms;
    } else {
        room.black_remaining_ms = room.black_remaining_ms.saturating_sub(elapsed_ms);
        room.black_remaining_ms += room.increment_ms;
    }

    room.last_move_at = Some(now_ms);
    game_state.apply_move(move_notation)?;
    let game_state_clone = game_state.clone();
    room.add_move(player_id.to_string(), move_notation.to_string());

    let response = ServerMessage::MoveMade {
        room_id: room_id.to_string(),
        player_id: player_id.to_string(),
        move_notation: move_notation.to_string(),
        game_state: game_state_clone,
    };

    if let Some(sender) = state.message_senders.get(room_id) {
        let _ = sender.send(response.clone());
    }

    Ok(response)
}

pub fn leave_room(room_id: &str, player_id: &str) -> Result<ServerMessage, String> {
    let mut state = GAME_STATE.lock().unwrap();

    // Check if room exists and remove player
    let should_cleanup = {
        let room = state.rooms.get_mut(room_id).ok_or_else(|| "Room not found".to_string())?;
        if !room.remove_player(player_id) {
            return Err("Player not in room".to_string());
        }
        room.players.is_empty()
    };

    // Create response message
    let response = ServerMessage::PlayerLeft {
        room_id: room_id.to_string(),
        player_id: player_id.to_string(),
    };

    // Broadcast to all players in the room
    if let Some(sender) = state.message_senders.get(room_id) {
        let _ = sender.send(response.clone());
    }

    // Clean up empty rooms
    if should_cleanup {
        state.rooms.remove(room_id);
        state.message_senders.remove(room_id);
    }

    Ok(response)
}

// Get game log
pub fn get_game_log(room_id: &str) -> Result<ServerMessage, String> {
    let state = GAME_STATE.lock().unwrap();
    
    // Check if room exists
    let room = state.rooms.get(room_id).ok_or_else(|| "Room not found".to_string())?;
    
    // Create response message
    let response = ServerMessage::GameLog {
        room_id: room_id.to_string(),
        moves: room.moves.clone(),
    };
    
    Ok(response)
}

// Handle a takeback offer from a player.
// Current behavior: only board state and move history are affected; clocks/time controls are not modified.
pub fn offer_takeback(room_id: &str, player_id: &str) -> Result<ServerMessage, String> {
    let mut state = GAME_STATE.lock().unwrap();

    let room = state
        .rooms
        .get_mut(room_id)
        .ok_or_else(|| "Room not found".to_string())?;

    // Ensure player is in the room
    if !room.players.iter().any(|p| p.id == player_id) {
        return Err("Player not in room".to_string());
    }

    // Require at least one full move (two half-moves) to be able to take back
    if room.moves.len() < 2 {
        return Err("Not enough moves to take back".to_string());
    }

    // Only one pending takeback at a time
    if room.pending_takeback.is_some() {
        return Err("A takeback request is already pending".to_string());
    }

    room.pending_takeback = Some(player_id.to_string());

    let response = ServerMessage::TakebackOffered {
        room_id: room_id.to_string(),
        requester_id: player_id.to_string(),
    };

    if let Some(sender) = state.message_senders.get(room_id) {
        let _ = sender.send(response.clone());
    }

    Ok(response)
}

// Accept a pending takeback request and roll back one full move (two half-moves).
pub fn accept_takeback(room_id: &str, player_id: &str) -> Result<ServerMessage, String> {
    let mut state = GAME_STATE.lock().unwrap();

    let room = state
        .rooms
        .get_mut(room_id)
        .ok_or_else(|| "Room not found".to_string())?;

    // Ensure player is in the room
    if !room.players.iter().any(|p| p.id == player_id) {
        return Err("Player not in room".to_string());
    }

    // There must be a pending takeback request
    let requester_id = match &room.pending_takeback {
        Some(id) => id.clone(),
        None => return Err("No pending takeback request".to_string()),
    };

    // Only the other player (not requester) can accept
    if requester_id == player_id {
        return Err("Requester cannot accept their own takeback".to_string());
    }

    // Need at least one full move (two half-moves) to roll back
    if room.moves.len() < 2 {
        return Err("Not enough moves to take back".to_string());
    }

    // Truncate last two half-moves
    let new_len = room.moves.len() - 2;
    room.moves.truncate(new_len);

    // Rebuild game state from initial position and remaining moves
    let mut game_state = GameState::new_game();
    for mv in &room.moves {
        game_state.apply_move(&mv.move_notation)?;
    }

    room.game_state = Some(game_state.clone());
    room.pending_takeback = None;

    let response = ServerMessage::TakebackAccepted {
        room_id: room_id.to_string(),
        game_state,
        moves: room.moves.clone(),
    };

    if let Some(sender) = state.message_senders.get(room_id) {
        let _ = sender.send(response.clone());
    }

    Ok(response)
}

// Reject a pending takeback request.
pub fn reject_takeback(room_id: &str, player_id: &str) -> Result<ServerMessage, String> {
    let mut state = GAME_STATE.lock().unwrap();

    let room = state
        .rooms
        .get_mut(room_id)
        .ok_or_else(|| "Room not found".to_string())?;

    // Ensure player is in the room
    if !room.players.iter().any(|p| p.id == player_id) {
        return Err("Player not in room".to_string());
    }

    // There must be a pending takeback request
    if room.pending_takeback.is_none() {
        return Err("No pending takeback request".to_string());
    }

    room.pending_takeback = None;

    let response = ServerMessage::TakebackRejected {
        room_id: room_id.to_string(),
        by_player_id: player_id.to_string(),
    };

    if let Some(sender) = state.message_senders.get(room_id) {
        let _ = sender.send(response.clone());
    }

    Ok(response)
}

// Database integration functions
// These are placeholders for future implementation

pub fn save_game_to_db(_room_id: &str) -> Result<(), String> {
    // In a real implementation, this would save the game state to a database
    Ok(())
}

pub fn load_game_from_db(_room_id: &str) -> Result<Room, String> {
    // In a real implementation, this would load the game state from a database
    Err("Not implemented".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn cleanup_room(room_id: &str) {
        let mut state = GAME_STATE.lock().unwrap();
        state.rooms.remove(room_id);
        state.message_senders.remove(room_id);
    }

    #[test]
    fn test_move_within_time() {
        let room_id = create_room_with_time(10_000, 0);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        let result = send_move(&room_id, "white_player", "e2e4");
        assert!(result.is_ok());
        cleanup_room(&room_id);
    }

    #[test]
    fn test_move_after_flag_fall() {
        let room_id = create_room_with_time(1000, 0);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        thread::sleep(Duration::from_millis(2000));
        let result = send_move(&room_id, "white_player", "e2e4");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Time expired"));
        cleanup_room(&room_id);
    }

    #[test]
    fn test_move_within_latency_buffer() {
        let room_id = create_room_with_time(500, 0);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        thread::sleep(Duration::from_millis(800));
        let result = send_move(&room_id, "white_player", "e2e4");
        assert!(result.is_ok());
        cleanup_room(&room_id);
    }

    #[test]
    fn test_move_after_latency_buffer() {
        let room_id = create_room_with_time(500, 0);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        thread::sleep(Duration::from_millis(1500));
        let result = send_move(&room_id, "white_player", "e2e4");
        assert!(result.is_err());
        cleanup_room(&room_id);
    }

    #[test]
    fn test_clock_deduction() {
        let room_id = create_room_with_time(10_000, 0);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        thread::sleep(Duration::from_millis(100));
        send_move(&room_id, "white_player", "e2e4").unwrap();
        let state = GAME_STATE.lock().unwrap();
        let room = state.rooms.get(&room_id).unwrap();
        assert!(room.white_remaining_ms < 10_000);
        assert_eq!(room.black_remaining_ms, 10_000);
        drop(state);
        cleanup_room(&room_id);
    }

    #[test]
    fn test_increment_applied() {
        let room_id = create_room_with_time(10_000, 2_000);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        
        // Capture initial time before the move
        let initial_time = {
            let state = GAME_STATE.lock().unwrap();
            let room = state.rooms.get(&room_id).unwrap();
            room.white_remaining_ms
        };
        
        send_move(&room_id, "white_player", "e2e4").unwrap();
        
        let state = GAME_STATE.lock().unwrap();
        let room = state.rooms.get(&room_id).unwrap();
        
        // Verify increment was applied (should be greater than initial despite time passing)
        assert!(
            room.white_remaining_ms > initial_time,
            "Expected white's time to increase after move with increment. Initial: {}, Final: {}",
            initial_time,
            room.white_remaining_ms
        );
        
        drop(state);
        cleanup_room(&room_id);
    }

    #[test]
    fn test_game_timeout_status() {
        let room_id = create_room_with_time(100, 0);
        join_room(&room_id, "white_player", None).unwrap();
        join_room(&room_id, "black_player", None).unwrap();
        thread::sleep(Duration::from_millis(1000));
        let _ = send_move(&room_id, "white_player", "e2e4");
        let state = GAME_STATE.lock().unwrap();
        let room = state.rooms.get(&room_id).unwrap();
        let game_state = room.game_state.as_ref().unwrap();
        assert!(matches!(game_state.status, GameStatus::Timeout));
        drop(state);
        cleanup_room(&room_id);
    }
}