use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use futures::future::{ok, LocalBoxFuture, Ready};
use std::task::{Context, Poll};
use tracing::Span;
use uuid::Uuid;

pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareImpl<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdMiddlewareImpl { service })
    }
}

pub struct RequestIdMiddlewareImpl<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddlewareImpl<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let request_id = req
            .headers()
            .get("X-Request-ID")
            .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let span = tracing::info_span!("request", request_id = %request_id);
        let _enter = span.enter();

        req.extensions_mut().insert(request_id.clone());

        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;
            res.headers_mut().insert(
                "X-Request-ID".parse().unwrap(),
                request_id.parse().unwrap(),
            );
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App, HttpResponse};

    #[actix_web::test]
    async fn test_request_id_generated_when_missing() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", actix_web::web::get().to(|| async { HttpResponse::Ok() })),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        let request_id = resp.headers().get("X-Request-ID");
        assert!(request_id.is_some());
        let id_str = request_id.unwrap().to_str().unwrap();
        assert!(!id_str.is_empty());
        // Should be a valid UUID
        assert!(Uuid::parse_str(id_str).is_ok());
    }

    #[actix_web::test]
    async fn test_request_id_passed_through() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", actix_web::web::get().to(|| async { HttpResponse::Ok() })),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("X-Request-ID", "custom-id-123"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        let request_id = resp.headers().get("X-Request-ID").unwrap().to_str().unwrap();
        assert_eq!(request_id, "custom-id-123");
    }

    #[actix_web::test]
    async fn test_request_id_returned_in_response() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", actix_web::web::get().to(|| async { HttpResponse::Ok() })),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.headers().contains_key("X-Request-ID"));
    }
}
