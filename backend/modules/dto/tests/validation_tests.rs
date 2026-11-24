use dto::auth::{LoginRequest, RegisterRequest};
use dto::games::{CreateGameRequest, ImportGameRequest, MakeMoveRequest};
use dto::players::{NewPlayer, UpdatePlayer};
use validator::Validate;

#[test]
fn register_request_boundaries() {
    let register = |username: &str, email: &str, password: &str| RegisterRequest {
        username: username.to_string(),
        email: email.to_string(),
        password: password.to_string(),
    };

    assert!(register("abc", "user@example.com", "12345678").validate().is_ok());
    assert!(register("ab", "user@example.com", "12345678").validate().is_err());
    assert!(register("abc", "not-an-email", "12345678").validate().is_err());
    assert!(register("abc", "user@example.com", "short").validate().is_err());
}

#[test]
fn login_request_validators() {
    let valid = LoginRequest {
        username: "chess_master".to_string(),
        password: "SecurePass123!".to_string(),
    };
    assert!(valid.validate().is_ok());

    let invalid = LoginRequest {
        username: "ab".to_string(),
        password: "short".to_string(),
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn new_player_validators() {
    assert!(NewPlayer::test_player().validate().is_ok());
    assert!(NewPlayer::invalid_player(dto::players::InvalidPlayer::Username)
        .validate()
        .is_err());
    assert!(NewPlayer::invalid_player(dto::players::InvalidPlayer::Email)
        .validate()
        .is_err());
    assert!(NewPlayer::invalid_player(dto::players::InvalidPlayer::Password)
        .validate()
        .is_err());
}

#[test]
fn update_player_username_is_optional_but_bounded() {
    assert!(UpdatePlayer {
        username: None,
        real_name: None,
        biography: None,
        country: None,
        flair: None,
        location: None,
        fide_rating: None,
        social_links: None,
    }
    .validate()
    .is_ok());

    let invalid = UpdatePlayer {
        username: Some("abc".to_string()),
        real_name: None,
        biography: None,
        country: None,
        flair: None,
        location: None,
        fide_rating: None,
        social_links: None,
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn game_request_validators() {
    let create_game = |time_control: i32, increment: i32| CreateGameRequest {
        time_control,
        increment,
        player_color: None,
        opponent_id: None,
    };
    assert!(create_game(60, 0).validate().is_ok());
    assert!(create_game(59, 61).validate().is_err());

    assert!(MakeMoveRequest {
        chess_move: "e2e4".to_string()
    }
    .validate()
    .is_ok());
    assert!(MakeMoveRequest {
        chess_move: "not a move".to_string()
    }
    .validate()
    .is_err());

    assert!(ImportGameRequest {
        pgn: "1. e4 e5 2. Nf3 Nc6".to_string()
    }
    .validate()
    .is_ok());
    assert!(ImportGameRequest {
        pgn: "too short".to_string()
    }
    .validate()
    .is_err());
}
