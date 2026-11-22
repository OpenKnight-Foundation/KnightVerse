use chrono::{Duration, Utc};
use db_entity::refresh_token;
use sea_orm::{DbBackend, MockDatabase, MockExecResult};
use security::token_service::{TokenService, TokenServiceError};
use uuid::Uuid;

fn token_model(used_at: Option<chrono::DateTime<Utc>>) -> refresh_token::Model {
    let now = Utc::now();
    refresh_token::Model {
        id: Uuid::new_v4(),
        player_id: 7,
        family_id: Uuid::new_v4(),
        token_hash: "hash".to_string(),
        created_at: now,
        used_at,
        expires_at: now + Duration::days(7),
        is_revoked: false,
    }
}

#[test]
fn hash_token_is_deterministic_and_distinct() {
    assert_eq!(TokenService::hash_token("abc"), TokenService::hash_token("abc"));
    assert_ne!(TokenService::hash_token("abc"), TokenService::hash_token("abd"));
    assert_eq!(TokenService::hash_token("abc").len(), 64);
}

#[tokio::test]
async fn verify_marks_token_used_and_returns_family() {
    let model = token_model(None);
    let family_id = model.family_id;
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results(vec![vec![model]])
        .append_exec_results(vec![MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }])
        .into_connection();

    let result = TokenService::verify_and_mark_used(&db, "token", 7).await.unwrap();
    assert_eq!(result, family_id);
}

#[tokio::test]
async fn reuse_detection_invalidates_the_family() {
    let model = token_model(Some(Utc::now()));
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results(vec![vec![model]])
        .append_exec_results(vec![MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }])
        .into_connection();

    let result = TokenService::verify_and_mark_used(&db, "token", 7).await;
    assert!(matches!(result, Err(TokenServiceError::TokenReuseDetected)));
}

#[test]
fn error_messages_are_stable() {
    assert_eq!(
        TokenServiceError::TokenNotFound.to_string(),
        "Token not found"
    );
    assert!(TokenServiceError::TokenReuseDetected
        .to_string()
        .contains("theft"));
}
