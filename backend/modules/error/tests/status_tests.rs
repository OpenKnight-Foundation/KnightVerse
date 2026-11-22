use error::error::ApiError;
use serde_json::Value;
use validator::ValidationErrors;

async fn body_json(error: ApiError) -> (u16, Value) {
    let response = error.error_response();
    let status = response.status().as_u16();
    let bytes = actix_web::body::to_bytes(response.into_body()).await.unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    (status, json)
}

#[tokio::test]
async fn client_error_variants_map_to_4xx() {
    let (status, body) = body_json(ApiError::InvalidCredentials).await;
    assert_eq!(status, 400);
    assert_eq!(body["code"], 400);
    assert!(body["error"].as_str().unwrap().contains("credentials"));

    let (status, body) = body_json(ApiError::BadRequest("nope".to_string())).await;
    assert_eq!(status, 400);
    assert_eq!(body["error"], "nope");

    let (status, body) = body_json(ApiError::Forbidden("denied".to_string())).await;
    assert_eq!(status, 403);
    assert_eq!(body["code"], 403);

    let (status, _) = body_json(ApiError::NotFound("Game".to_string())).await;
    assert_eq!(status, 404);

    let (status, _) = body_json(ApiError::PgnParseError("bad".to_string())).await;
    assert_eq!(status, 400);

    let (status, _) = body_json(ApiError::IllegalMoveError {
        move_number: 3,
        move_text: "e2e5".to_string(),
        reason: "illegal".to_string(),
    })
    .await;
    assert_eq!(status, 422);

    let (status, _) = body_json(ApiError::ValidationError(ValidationErrors::new())).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn server_error_variants_map_to_5xx() {
    let (status, body) =
        body_json(ApiError::DatabaseError(sea_orm::DbErr::Custom("down".to_string()))).await;
    assert_eq!(status, 500);
    assert_eq!(body["code"], 500);

    let (status, _) =
        body_json(ApiError::PasswordHashError(argon2::password_hash::Error::Password)).await;
    assert_eq!(status, 500);

    let (status, _) = body_json(ApiError::NotImplemented("soon".to_string())).await;
    assert_eq!(status, 501);
}
