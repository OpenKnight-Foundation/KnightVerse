# Distributed tracing (BE-183)

The backend exports OpenTelemetry traces over OTLP/HTTP when
`OTEL_EXPORTER_OTLP_ENDPOINT` is set. Logging keeps working either way
(pretty in debug builds, JSON in release).

## Enabling export

Set the exporter endpoint to the base URL of your OTel collector:

```sh
export OTEL_EXPORTER_OTLP_ENDPOINT="http://otel-collector:4318"
# optional:
export OTEL_SERVICE_NAME="knightverse-api"      # default: knightverse-api
export OTEL_EXPORTER_OTLP_HEADERS="x-api-key=…" # auth, if any
```

The exporter reads the standard `OTEL_EXPORTER_OTLP_*` variables itself.
`/v1/traces` is appended to `OTEL_EXPORTER_OTLP_ENDPOINT` automatically.
When the variable is unset, export is off and the SDK is replaced by a no-op
provider — no network traffic, no measurable overhead.

## Request id ↔ trace id

Every HTTP request carries an `X-Request-ID` (`api::request_id`). The
request's root tracing span is parented under a remote span context whose
trace id is derived from that id:

* A UUID request id (the generated default) is used verbatim: trace id =
  the UUID bytes. `X-Request-ID: 3f2a6c1e-9b4d-4e2a-8c7f-0123456789ab` ↔
  trace id `3f2a6c1e9b4d4e2a8c7f0123456789ab`.
* Any other client-supplied id is mapped deterministically (SHA-256,
  first 16 bytes).

So a request id from a log line or a client report looks up directly in
the tracing backend.

## Instrumented components

* **HTTP requests**: `tracing_actix_web::TracingLogger` plus
  `RequestIdMiddleware` emit the root `request` span.
* **PostgreSQL**: every SQL statement is exported as a `db <OPERATION>`
  client span by `db::tracing_spans::record_query_span`, which is hooked
  onto every sea-orm connection in `DbPool`.
* **Redis**: `telemetry::redis_span(command)` returns a client span to
  `.instrument(…)` an individual `redis::cmd(...).query_async(...)` future
  with when you need per-command visibility.

## Local verification

Run an OTel collector that prints to stdout, point the exporter at it and
hit the API:

```sh
docker run --rm -p 4318:4318 \
  -v $PWD/backend/docs/otel-collector.yaml:/etc/otelcol/config.yaml \
  otel/opentelemetry-collector:latest

OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 \
  cargo run -p api
```

`OpenTelemetry OTLP trace export enabled` appears once at startup;
request/DB/Redis spans show up in the collector tagged with the
`X-Request-ID`-derived trace id.
