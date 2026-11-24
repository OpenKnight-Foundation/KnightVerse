//! BE-183: OpenTelemetry client spans for SQL statements.
//!
//! sea-orm calls a metric callback after every statement with its elapsed
//! time. [`record_query_span`] turns that into an OpenTelemetry `CLIENT` span
//! with the statement's real start and end time, parented to the current
//! `tracing` span (normally the HTTP request span), so database time shows up
//! inside the request's trace.
//!
//! The span goes through the global tracer provider. When OTLP export is not
//! configured that provider is the no-op default, so this costs one clock read
//! per statement.

use std::borrow::Cow;
use std::time::SystemTime;

use opentelemetry::trace::{Span as _, SpanKind, Status, Tracer as _};
use opentelemetry::{global, KeyValue};
use sea_orm::metric::Info;
use tracing_opentelemetry::OpenTelemetrySpanExt;

/// Instrumentation scope name for database spans.
pub const DB_TRACER_NAME: &str = "knightverse-db";

/// Longest SQL text recorded on a span. Statements are parameterised, so this
/// never contains bound values, but very long statements are truncated.
pub const MAX_STATEMENT_LEN: usize = 1024;

/// The SQL verb (`SELECT`, `INSERT`, ...) used for the span name.
pub fn operation_name(sql: &str) -> Cow<'static, str> {
    match sql.split_whitespace().next() {
        Some(word) if !word.is_empty() => Cow::Owned(word.to_ascii_uppercase()),
        _ => Cow::Borrowed("QUERY"),
    }
}

/// Truncates `sql` to at most [`MAX_STATEMENT_LEN`] bytes on a char boundary.
pub fn truncate_statement(sql: &str) -> &str {
    if sql.len() <= MAX_STATEMENT_LEN {
        return sql;
    }
    let mut end = MAX_STATEMENT_LEN;
    while !sql.is_char_boundary(end) {
        end -= 1;
    }
    &sql[..end]
}

/// sea-orm metric callback: records one client span for the finished statement.
pub fn record_query_span(info: &Info<'_>) {
    let end = SystemTime::now();
    let start = end.checked_sub(info.elapsed).unwrap_or(end);
    let sql = info.statement.sql.as_str();
    let operation = operation_name(sql);

    let tracer = global::tracer(DB_TRACER_NAME);
    let parent = tracing::Span::current().context();
    let mut span = tracer
        .span_builder(format!("db {operation}"))
        .with_kind(SpanKind::Client)
        .with_start_time(start)
        .with_attributes([
            KeyValue::new("db.system.name", "postgresql"),
            KeyValue::new("db.operation.name", operation.into_owned()),
            KeyValue::new("db.query.text", truncate_statement(sql).to_owned()),
        ])
        .start_with_context(&tracer, &parent);

    if info.failed {
        span.set_status(Status::error("query failed"));
    }
    span.end_with_timestamp(end);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_name_is_the_uppercased_sql_verb() {
        assert_eq!(operation_name("select * from users"), "SELECT");
        assert_eq!(operation_name("  INSERT INTO games VALUES ($1)"), "INSERT");
        assert_eq!(operation_name(""), "QUERY");
        assert_eq!(operation_name("   "), "QUERY");
    }

    #[test]
    fn truncate_statement_respects_char_boundaries() {
        let short = "SELECT 1";
        assert_eq!(truncate_statement(short), short);

        let long = format!("SELECT '{}'", "é".repeat(MAX_STATEMENT_LEN));
        let truncated = truncate_statement(&long);
        assert!(truncated.len() <= MAX_STATEMENT_LEN);
        assert!(long.starts_with(truncated));
    }
}
