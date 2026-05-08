#[cfg(test)]
mod rate_limit;

#[cfg(test)]
mod tests {
    use actix_web::{App, dev::Service, http::StatusCode, test, web};
    use dto::players::{InvalidPlayer, NewPlayer};

    use crate::players::add_player;

    #[actix_web::test]
    async fn test_index_post_no_body() {
            std::env::set_var("TEST_NO_DB", "1");
        let app =
            test::init_service(App::new().service(web::scope("/v1/players").service(add_player)))
                .await;
        let req = test::TestRequest::post().uri("/v1/players").to_request();
        let res = app.call(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_index_post_with_body() {
            std::env::set_var("TEST_NO_DB", "1");
        let app =
            test::init_service(App::new().service(web::scope("/v1/players").service(add_player)))
                .await;
        let req = test::TestRequest::post()
            .uri("/v1/players")
            .set_json(NewPlayer::test_player())
            .to_request();
        let res = app.call(req).await.unwrap();
        let status = res.status();
        let body = test::read_body(res).await;
        let response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Verify response structure
        assert!(
            response.get("data").is_some(),
            "Response should contain a 'data' field"
        );
        let data = &response["data"];
        println!("{:?}", data);

        assert!(
            data.get("id").is_some(),
            "Response should contain player ID"
        );
        assert!(
            data.get("username").is_some(),
            "Response should contain username"
        );
        assert!(
            data.get("email").is_some(),
            "Response should contain email address"
        );
        assert!(
            data.get("real_name").is_some(),
            "Response should contain 'real_name'"
        );
        assert_eq!(status, StatusCode::OK);
    }

    #[actix_web::test]
    async fn test_index_post_with_invalid_username() {
            std::env::set_var("TEST_NO_DB", "1");
        let app =
            test::init_service(App::new().service(web::scope("/v1/players").service(add_player)))
                .await;
        let req = test::TestRequest::post()
            .uri("/v1/players")
            .set_json(NewPlayer::invalid_player(InvalidPlayer::Username))
            .to_request();
        let res = app.call(req).await.unwrap();
        let status = res.status();
        let body = test::read_body(res).await;
        let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            error_response.get("error").is_some(),
            "Response should contain an 'error' field"
        );
        assert!(
            error_response.get("code").is_some(),
            "Response should contain a 'code' field"
        );
        let error = error_response["error"].as_str().unwrap();
        assert!(
            error.contains("Username"),
            "Error should mention the username field"
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_index_post_with_invalid_email() {
            std::env::set_var("TEST_NO_DB", "1");
        let app =
            test::init_service(App::new().service(web::scope("/v1/players").service(add_player)))
                .await;
        let req = test::TestRequest::post()
            .uri("/v1/players")
            .set_json(NewPlayer::invalid_player(InvalidPlayer::Email))
            .to_request();
        let res = app.call(req).await.unwrap();
        let status = res.status();
        let body = test::read_body(res).await;
        let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            error_response.get("error").is_some(),
            "Response should contain an 'error' field"
        );
        assert!(
            error_response.get("code").is_some(),
            "Response should contain a 'code' field"
        );
        let error = error_response["error"].as_str().unwrap();
        println!("{}", error);
        assert!(
            error.contains("email"),
            "Error should mention the email field"
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[actix_web::test]
    async fn test_index_post_with_invalid_password() {
            std::env::set_var("TEST_NO_DB", "1");
        let app =
            test::init_service(App::new().service(web::scope("/v1/players").service(add_player)))
                .await;
        let req = test::TestRequest::post()
            .uri("/v1/players")
            .set_json(NewPlayer::invalid_player(InvalidPlayer::Password))
            .to_request();
        let res = app.call(req).await.unwrap();
        let status = res.status();
        let body = test::read_body(res).await;
        let error_response: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(
            error_response.get("error").is_some(),
            "Response should contain an 'error' field"
        );
        assert!(
            error_response.get("code").is_some(),
            "Response should contain a 'code' field"
        );
        let error = error_response["error"].as_str().unwrap();
        assert!(
            error.contains("Password"),
            "Error should mention the password field"
        );
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}
