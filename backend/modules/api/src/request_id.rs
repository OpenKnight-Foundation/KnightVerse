
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{Error, HttpMessage};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use std::task::{Context, Poll};
use tracing::Instrument;
use uuid::Uuid;

const REQUEST_ID_HEADER: &str = "X-Request-ID";
const MAX_REQUEST_ID_LENGTH: usize = 128;

pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
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
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| is_valid_request_id(value))
            .map(str::to_owned)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let span = tracing::info_span!(
            "request",
            request_id = %request_id,
            method = %req.method(),
            path = %req.path(),
        );

        req.extensions_mut().insert(request_id.clone());

        let fut = self.service.call(req);

        Box::pin(
            async move {
                let mut res = fut.await?;

                // HeaderValue::from_str validates the value before insertion.
                if let Ok(value) = HeaderValue::from_str(&request_id) {
                    res.headers_mut().insert(
                        HeaderName::from_static(REQUEST_ID_HEADER.to_ascii_lowercase().leak()),
                        value,
                    );
                }

                Ok(res)
            }
            .instrument(span),
        )
    }
}

/// Accept non-empty, bounded, printable ASCII request IDs.
///
/// Restricting characters avoids control characters and newlines in
/// identifiers supplied by external clients.
fn is_valid_request_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_REQUEST_ID_LENGTH
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'_' | b'.' | b':')
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    #[actix_web::test]
    async fn generates_request_id_when_missing() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        let request_id = resp
            .headers()
            .get(REQUEST_ID_HEADER)
            .expect("request ID should be returned")
            .to_str()
            .expect("request ID should be valid ASCII");

        assert!(Uuid::parse_str(request_id).is_ok());
    }

    #[actix_web::test]
    async fn preserves_valid_incoming_request_id() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header((REQUEST_ID_HEADER, "custom-id-123"))
            .to_request();

        let resp = test::call_service(&app, req).await;

        let request_id = resp
            .headers()
            .get(REQUEST_ID_HEADER)
            .expect("request ID should be returned")
            .to_str()
            .unwrap();

        assert_eq!(request_id, "custom-id-123");
    }

    #[actix_web::test]
    async fn replaces_invalid_incoming_request_id() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route("/", web::get().to(|| async { HttpResponse::Ok().finish() })),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/")
            .insert_header((REQUEST_ID_HEADER, "invalid request id"))
            .to_request();

        let resp = test::call_service(&app, req).await;

        let request_id = resp
            .headers()
            .get(REQUEST_ID_HEADER)
            .expect("request ID should be returned")
            .to_str()
            .unwrap();

        assert!(Uuid::parse_str(request_id).is_ok());
    }

    #[actix_web::test]
    async fn exposes_request_id_through_extensions() {
        let app = test::init_service(
            App::new()
                .wrap(RequestIdMiddleware)
                .route(
                    "/",
                    web::get().to(|req: actix_web::HttpRequest| async move {
                        let request_id = req
                            .extensions()
                            .get::<String>()
                            .cloned()
                            .expect("request ID should be available in extensions");

                        HttpResponse::Ok().body(request_id)
                    }),
                ),
        )
        .await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        let response_id = test::read_body(resp).await;
        let response_id = std::str::from_utf8(&response_id).unwrap();

        assert!(Uuid::parse_str(response_id).is_ok());
    }
}
