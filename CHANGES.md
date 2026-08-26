# BE-26 & BE-27 Implementation

## BE-26: Graceful shutdown for Actix-Web server
**Location:** `backend/modules/api/src/server.rs` (invoked from `backend/src/main.rs`)

- Installs Unix SIGTERM and SIGINT handlers via `tokio::signal::unix`.
- On signal, calls `ServerHandle::stop(true)` so Actix drains in-flight
  requests / WebSocket connections and shuts down the worker thread pool
  cleanly instead of aborting mid-transaction.

## BE-27: Strict JWT secret and Redis URL environment variables
**Location:** `backend/modules/security/src/jwt.rs`, `backend/modules/api/src/server.rs`

- Added `JwtService::from_env()` which **requires** `JWT_SECRET` or
  `JWT_SECRET_KEY` (no hardcoded fallback). Panics on empty or known
  insecure default values.
- `REDIS_URL` is now required via `std::env::var(...).expect(...)` —
  no `redis://localhost:6379` fallback.
- `.env.example` updated to document required secrets.

### Required env vars at startup
| Variable | Required | Notes |
|----------|----------|-------|
| `JWT_SECRET` or `JWT_SECRET_KEY` | **Yes** | Prefer `JWT_SECRET`; rejects known insecure defaults |
| `REDIS_URL` | **Yes** | e.g. `redis://localhost:6379` |
| `DATABASE_URL` | Yes (already) | |
| `JWT_EXPIRATION_SECS` | No | Default 3600 |
