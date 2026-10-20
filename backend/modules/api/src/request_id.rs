// Middleware that attaches a unique request ID to every request/response,
// so logs and client reports can be correlated.
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpMessage};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use std::task::{Context, Poll};
use uuid::Uuid;

/// Factory type registered with `App::wrap(...)`.
/// Actix calls `new_transform` to wrap the inner service with the real middleware.
pub struct RequestIdMiddleware;

// `Transform` is the "middleware factory" trait: it builds the actual middleware
// service around the next service in the chain (`S`).
impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static, // response body type; must be 'static to live in the boxed future
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareImpl<S>;
    // Construction is synchronous and infallible, so an immediately-ready future is enough
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdMiddlewareImpl { service })
    }
}

/// The actual middleware service; holds the next service in the chain.
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
    // Boxed so we can return one concrete future type from `call`
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    // This middleware has no backpressure of its own, so defer readiness to the inner service
    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Reuse the client-supplied X-Request-ID if present, non-empty, and valid text;
        // otherwise generate a fresh UUID v4.
        let request_id = req
            .headers()
            .get("X-Request-ID")
            .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Create a tracing span tagged with the request ID for log correlation.
        // NOTE: the guard is dropped when `call` returns, so the span only covers this
        // synchronous part, not the async handling of the request below.
        let span = tracing::info_span!("request", request_id = %request_id);
        let _enter = span.enter();

        // Store the ID in request extensions so handlers/other middleware can read it
        req.extensions_mut().insert(request_id.clone());

        // Start the inner service; it runs when the future below is awaited
        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;
            // Echo the ID back on the response so clients can quote it in bug reports
            res.headers_mut()
                .insert("X-Request-ID".parse().unwrap(), request_id.parse().unwrap());
            Ok(res)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App, HttpResponse};

    // With no incoming header, the middleware should generate a valid UUID
    #[actix_web::test]
    async fn test_request_id_generated_when_missing() {
        // Minimal app: the middleware plus a handler that just returns 200
        let app = test::init_service(App::new().wrap(RequestIdMiddleware).route(
            "/",
            actix_web::web::get().to(|| async { HttpResponse::Ok() }),
        ))
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

    // A client-supplied ID should be preserved and echoed back unchanged
    #[actix_web::test]
    async fn test_request_id_passed_through() {
        let app = test::init_service(App::new().wrap(RequestIdMiddleware).route(
            "/",
            actix_web::web::get().to(|| async { HttpResponse::Ok() }),
        ))
        .await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header(("X-Request-ID", "custom-id-123"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        let request_id = resp
            .headers()
            .get("X-Request-ID")
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(request_id, "custom-id-123");
    }

    // Sanity check: the response always carries the header
    #[actix_web::test]
    async fn test_request_id_returned_in_response() {
        let app = test::init_service(App::new().wrap(RequestIdMiddleware).route(
            "/",
            actix_web::web::get().to(|| async { HttpResponse::Ok() }),
        ))
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.headers().contains_key("X-Request-ID"));
    }
}