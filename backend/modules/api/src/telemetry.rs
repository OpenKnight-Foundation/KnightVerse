//! BE-183: tracing setup with optional OpenTelemetry (OTLP) export.
//!
//! * Logging is unchanged: `RUST_LOG`-filtered, pretty in debug builds and
//!   JSON in release builds.
//! * When `OTEL_EXPORTER_OTLP_ENDPOINT` is set, spans are also exported over
//!   OTLP/HTTP (protobuf) by a batch processor. The exporter reads the
//!   standard `OTEL_EXPORTER_OTLP_*` variables itself, so the value is the
//!   collector base URL (e.g. `http://otel-collector:4318`); `/v1/traces` is
//!   appended automatically.
//! * The request id (`X-Request-ID`) is used as the trace id, so a request id
//!   from a log line or client report is directly searchable in the tracing
//!   backend. See [`request_parent_context`].
//!
//! Database statements are exported as client spans by `db::tracing_spans`;
//! Redis commands use [`redis_span`].

use opentelemetry::trace::{
    SpanContext, SpanId, TraceContextExt, TraceFlags, TraceId, TraceState, TracerProvider as _,
};
use opentelemetry::{global, Context};
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use sha2::{Digest, Sha256};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
use uuid::Uuid;

/// Enables OTLP export when set to a non-empty value.
pub const OTLP_ENDPOINT_ENV: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
/// Standard override for the exported `service.name`.
pub const SERVICE_NAME_ENV: &str = "OTEL_SERVICE_NAME";
/// `service.name` used when `OTEL_SERVICE_NAME` is not set.
pub const DEFAULT_SERVICE_NAME: &str = "knightverse-api";
/// Instrumentation scope name for spans bridged from `tracing`.
pub const TRACER_NAME: &str = "knightverse-api";

/// Keeps the tracer provider alive and flushes pending spans on drop.
#[must_use = "dropping the guard shuts down span export"]
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl TelemetryGuard {
    /// `true` when spans are being exported over OTLP.
    pub fn otlp_enabled(&self) -> bool {
        self.provider.is_some()
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            if let Err(err) = provider.shutdown() {
                eprintln!("failed to flush OpenTelemetry spans on shutdown: {err}");
            }
        }
    }
}

/// The configured OTLP endpoint, if export is enabled.
pub fn otlp_endpoint() -> Option<String> {
    std::env::var(OTLP_ENDPOINT_ENV)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn resource() -> Resource {
    let builder = Resource::builder();
    // Resource::builder() already honours OTEL_SERVICE_NAME / OTEL_RESOURCE_ATTRIBUTES.
    if std::env::var_os(SERVICE_NAME_ENV).is_some() {
        builder.build()
    } else {
        builder.with_service_name(DEFAULT_SERVICE_NAME).build()
    }
}

/// Builds a tracer provider that batches spans to the OTLP/HTTP exporter.
pub fn build_otlp_tracer_provider(
) -> Result<SdkTracerProvider, opentelemetry_otlp::ExporterBuildError> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()?;
    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource())
        .build())
}

/// Installs the global `tracing` subscriber (and, when configured, the OTLP
/// exporter). Keep the returned guard alive for the life of the process.
pub fn init_tracing() -> TelemetryGuard {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    #[cfg(debug_assertions)]
    let fmt_layer = tracing_subscriber::fmt::layer().pretty().boxed();
    #[cfg(not(debug_assertions))]
    let fmt_layer = tracing_subscriber::fmt::layer().json().boxed();

    let provider = otlp_endpoint().and_then(|endpoint| match build_otlp_tracer_provider() {
        Ok(provider) => Some((endpoint, provider)),
        Err(err) => {
            // Tracing is not up yet, so report straight to stderr.
            eprintln!("OTLP trace export disabled: could not build exporter for {endpoint}: {err}");
            None
        }
    });

    let otel_layer = provider.as_ref().map(|(_, provider)| {
        // Database spans are created through the global provider.
        global::set_tracer_provider(provider.clone());
        tracing_opentelemetry::layer().with_tracer(provider.tracer(TRACER_NAME))
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();

    if let Some((endpoint, _)) = &provider {
        tracing::info!(endpoint = %endpoint, "OpenTelemetry OTLP trace export enabled");
    }

    TelemetryGuard {
        provider: provider.map(|(_, provider)| provider),
    }
}

fn non_zero<const N: usize>(mut bytes: [u8; N]) -> [u8; N] {
    // All-zero ids are invalid in OpenTelemetry.
    if bytes.iter().all(|byte| *byte == 0) {
        bytes[N - 1] = 1;
    }
    bytes
}

/// Maps a request id to an OpenTelemetry trace id.
///
/// A UUID request id (the generated default) is used verbatim: its 16 bytes
/// are the trace id, so `X-Request-ID: 3f2a…` is trace `3f2a…` with the
/// dashes removed. Any other client-supplied id is hashed (SHA-256, first 16
/// bytes) so the mapping is still deterministic.
pub fn trace_id_for_request_id(request_id: &str) -> TraceId {
    let bytes = match Uuid::parse_str(request_id) {
        Ok(uuid) if !uuid.is_nil() => *uuid.as_bytes(),
        _ => {
            let digest = Sha256::digest(request_id.as_bytes());
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&digest[..16]);
            bytes
        }
    };
    TraceId::from_bytes(non_zero(bytes))
}

/// Parent context that places a request's spans in the trace named by its
/// request id.
///
/// OpenTelemetry assigns trace ids to root spans itself, so the request span
/// is given a sampled remote parent carrying the request-derived trace id. The
/// parent span id is derived from the request id too; it stands for the
/// caller and is never exported.
pub fn request_parent_context(request_id: &str) -> Context {
    let digest = Sha256::digest(format!("span:{request_id}").as_bytes());
    let mut span_bytes = [0u8; 8];
    span_bytes.copy_from_slice(&digest[..8]);

    let span_context = SpanContext::new(
        trace_id_for_request_id(request_id),
        SpanId::from_bytes(non_zero(span_bytes)),
        TraceFlags::SAMPLED,
        true,
        TraceState::default(),
    );
    Context::new().with_remote_span_context(span_context)
}

/// Client span for one Redis command. Wrap the command's future with
/// `.instrument(redis_span("INCR"))`.
pub fn redis_span(command: &'static str) -> tracing::Span {
    tracing::info_span!(
        "redis",
        otel.name = %format_args!("redis {command}"),
        otel.kind = "client",
        db.system.name = "redis",
        db.operation.name = command,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request_id::RequestIdMiddleware;
    // Renamed so it doesn't shadow the built-in #[test] used by the sync tests.
    use actix_web::{test as actix_test, web, App, HttpResponse};
    use opentelemetry::trace::SpanKind;
    use opentelemetry_sdk::trace::InMemorySpanExporter;
    use tracing::Instrument;

    #[test]
    fn uuid_request_ids_become_the_trace_id_verbatim() {
        let request_id = "3f2a6c1e-9b4d-4e2a-8c7f-0123456789ab";
        assert_eq!(
            trace_id_for_request_id(request_id).to_string(),
            "3f2a6c1e9b4d4e2a8c7f0123456789ab"
        );
    }

    #[test]
    fn other_request_ids_map_deterministically() {
        let a = trace_id_for_request_id("custom-id-123");
        assert_eq!(a, trace_id_for_request_id("custom-id-123"));
        assert_ne!(a, trace_id_for_request_id("custom-id-124"));
        assert_ne!(a, TraceId::INVALID);
        // The nil UUID would be an invalid (all-zero) trace id, so it is hashed.
        assert_ne!(
            trace_id_for_request_id("00000000-0000-0000-0000-000000000000"),
            TraceId::INVALID
        );
    }

    #[test]
    fn request_parent_context_is_a_valid_sampled_remote_parent() {
        let cx = request_parent_context("custom-id-123");
        let span = cx.span();
        let parent = span.span_context();
        assert!(parent.is_valid());
        assert!(parent.is_remote());
        assert!(parent.is_sampled());
        assert_eq!(parent.trace_id(), trace_id_for_request_id("custom-id-123"));
    }

    #[test]
    fn otlp_endpoint_ignores_blank_values() {
        // Only this test touches the variable.
        std::env::set_var(OTLP_ENDPOINT_ENV, "   ");
        assert_eq!(otlp_endpoint(), None);
        std::env::set_var(OTLP_ENDPOINT_ENV, " http://collector:4318 ");
        assert_eq!(otlp_endpoint().as_deref(), Some("http://collector:4318"));
        std::env::remove_var(OTLP_ENDPOINT_ENV);
        assert_eq!(otlp_endpoint(), None);
    }

    /// End to end: an HTTP request's spans (including a Redis span created by
    /// the handler) are exported in the trace named by its X-Request-ID.
    #[actix_web::test]
    async fn request_spans_use_the_request_id_as_trace_id() {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("test")));
        let _default = tracing::subscriber::set_default(subscriber);

        let app = actix_test::init_service(App::new().wrap(RequestIdMiddleware).route(
            "/",
            web::get().to(|| async {
                async {}.instrument(redis_span("GET")).await;
                HttpResponse::Ok().finish()
            }),
        ))
        .await;

        let request_id = "3f2a6c1e-9b4d-4e2a-8c7f-0123456789ab";
        let req = actix_test::TestRequest::get()
            .uri("/")
            .insert_header(("X-Request-ID", request_id))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        provider.force_flush().expect("flush spans");
        let spans = exporter.get_finished_spans().expect("finished spans");
        let expected = trace_id_for_request_id(request_id);

        let request_span = spans
            .iter()
            .find(|span| span.name == "request")
            .expect("request span exported");
        assert_eq!(request_span.span_context.trace_id(), expected);

        let redis = spans
            .iter()
            .find(|span| span.name == "redis GET")
            .expect("redis span exported");
        assert_eq!(redis.span_context.trace_id(), expected);
        assert_eq!(redis.span_kind, SpanKind::Client);
        assert_eq!(redis.parent_span_id, request_span.span_context.span_id());
    }
}
