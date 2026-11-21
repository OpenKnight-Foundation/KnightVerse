use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use security::jwt::{JwtService, TokenType};
use uuid::Uuid;

const SECRET: &str = "unit_test_secret_for_jwt_roundtrips";

fn service() -> JwtService {
    JwtService::new(SECRET.to_string(), 3600)
}

#[test]
fn access_token_roundtrip() {
    let svc = service();
    let player_id = Uuid::new_v4();
    let token = svc.generate_token(7, "alice", player_id).unwrap();
    let claims = svc.validate_token(&token).unwrap();

    assert_eq!(claims.user_id, 7);
    assert_eq!(claims.username, "alice");
    assert_eq!(claims.player_id, player_id);
    assert_eq!(claims.token_type, TokenType::Access);
    assert!(claims.jti.is_some());
}

#[test]
fn reconnect_token_roundtrip_has_reconnect_type() {
    let svc = service();
    let token = svc
        .generate_reconnect_token(7, "alice", Uuid::new_v4(), "session-1")
        .unwrap();
    let claims = svc.validate_token(&token).unwrap();
    assert_eq!(claims.token_type, TokenType::Reconnect);
    assert_eq!(claims.jti.as_deref(), Some("session-1"));
}

#[test]
fn wrong_secret_is_rejected() {
    let token = service().generate_token(1, "alice", Uuid::new_v4()).unwrap();
    let other = JwtService::new("a_completely_different_secret".to_string(), 3600);
    assert!(other.validate_token(&token).is_err());
}

#[test]
fn tampered_payload_is_rejected() {
    let token = service().generate_token(1, "alice", Uuid::new_v4()).unwrap();
    let mut parts: Vec<String> = token.split('.').map(str::to_string).collect();
    parts[1].push('A');
    let tampered = parts.join(".");
    assert!(service().validate_token(&tampered).is_err());
}

#[test]
fn expired_token_is_rejected() {
    use chrono::Utc;
    let now = Utc::now().timestamp() as usize;
    let claims = security::jwt::Claims {
        sub: "1".to_string(),
        user_id: 1,
        player_id: Uuid::new_v4(),
        username: "alice".to_string(),
        exp: now - 10,
        iat: now - 100,
        jti: None,
        token_type: TokenType::Access,
    };
    let token = encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(SECRET.as_ref()),
    )
    .unwrap();
    assert!(service().validate_token(&token).is_err());
}

#[test]
fn header_extraction() {
    assert_eq!(
        JwtService::extract_token_from_header("Bearer abc.def.ghi"),
        Some("abc.def.ghi".to_string())
    );
    assert_eq!(JwtService::extract_token_from_header("Basic abc"), None);
    assert_eq!(JwtService::extract_token_from_header("Bearer "), Some(String::new()));
}
