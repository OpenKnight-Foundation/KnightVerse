# WebSocket move-submission load test

`ws-move-loadtest` is a small Tokio harness that exercises the **WebSocket
move-submission path** under realistic concurrent load: many simultaneous
games, each with two player sockets and any number of spectators, submitting
moves at a configurable rate while recording latency percentiles, delivery
counts, and an error rate.

It exists because the backend has unit and integration tests that cover the
socket one connection at a time, but nothing that reproduces "many games × many
spectators" load — so a regression in the move fan-out leg has no automated way
to be caught before production.

## Layout

```
loadtest/
├── README.md          # this file
├── BASELINE.md        # recorded runs + how to record a new one
└── ws-move-loadtest/  # the harness (its own cargo workspace, see below)
    ├── Cargo.toml
    └── src/{main.rs, lib.rs, mock.rs}
```

The harness is deliberately **its own cargo workspace** (`[workspace]` in its
`Cargo.toml`). It needs client-side networking crates the server never uses, and
keeping it out of `backend/Cargo.toml` means it can neither change the server's
dependency graph nor force a `backend/Cargo.lock` regeneration.

## Quick start

### 1. Against a local dev backend

```bash
# 1. Postgres + Redis (see backend/QUICK_START.md for the full walkthrough)
docker compose -f backend/docker-compose.monitoring.yml up -d postgres redis

# 2. The API server. JWT_SECRET must be a real secret — the server refuses
#    to boot on a known placeholder.
cd backend
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/knightverse
export REDIS_URL=redis://localhost:6379
export JWT_SECRET="$(openssl rand -hex 32)"
cargo run -p api          # listens on 127.0.0.1:8080 by default

# 3. The load test (separate terminal). Tokens are minted per socket with the
#    same JWT_SECRET, so each connection gets its own user/player identity.
cd backend/loadtest/ws-move-loadtest
export JWT_SECRET="<the same secret as the server>"
cargo run --release -- --games 20 --spectators-per-game 10 --moves 100 --rate 50
```

If you would rather not hand the harness a signing secret, log in normally and
pass a token: `--token "$(curl -s -XPOST localhost:8080/v1/auth/login -H 'content-type: application/json' -d '{"username":"...","password":"..."}' | jq -r .access_token)"`.
Reusing one token means every socket shares a `player_id`, which is fine for
broadcast timing but makes the `ConnectionStateTracker` see one player.

### 2. Self-test (no backend, no Postgres, no Redis)

`--self-test` starts an in-process mock that speaks the same contract — the same
route, the same `Authorization: Bearer` check, and the same "fan out
`Move`/`Clock`/`End` to every socket in the game, sender included" behaviour as
`LobbyState::Broadcast`. Use it to verify a harness change or to wire a smoke
check into CI:

```bash
cd backend/loadtest/ws-move-loadtest
cargo test                                    # 11 unit/integration tests, incl. an
                                             # end-to-end run against the mock
cargo run --release -- --self-test --games 10 --spectators-per-game 5 --moves 50 --rate 20
```

The mock is not a chess server — it relays frames, it does not validate them.
It is only there so the harness itself can be run deterministically.

## Options

```
--url <URL>                  Base WebSocket URL            [default: ws://127.0.0.1:8080]
--games <N>                  Concurrent games              [default: 10]
--players-per-game <N>       Player sockets per game       [default: 2]
--spectators-per-game <N>    Spectator sockets per game    [default: 0]
--moves <N>                  Moves per game                [default: 50]
--rate <R>                   Moves/s per game, 0 = flat out [default: 10]
--token <JWT>                Reuse one access token
--jwt-secret <SECRET>        Mint tokens (falls back to $JWT_SECRET / $JWT_SECRET_KEY)
--connect-timeout-ms <MS>    Handshake timeout             [default: 5000]
--drain-timeout-ms <MS>      Wait for the final broadcasts [default: 2000]
--max-error-rate <F>         Non-zero exit above this rate [default: 0.0]
--json <PATH>                Also write the report as JSON
--self-test                  Use the in-process mock endpoint
```

## What is measured — and what is not

Each game opens two player sockets (index 0 submits, index 1 observes) plus
`--spectators-per-game` read-only sockets (`?role=spectator`). Every move is
tagged with a unique `san` marker, and the harness timestamps each observation
of that marker:

| Stage | Meaning |
| --- | --- |
| `connect (handshake)` | TCP + HTTP upgrade + JWT check for one socket |
| `move -> opponent peer` | write the `Move` frame → the other player socket observes it |
| `move -> spectator` | write the `Move` frame → a spectator socket observes it |

The reported values are `p50/p90/p95/p99/max` in milliseconds plus the counts,
so a regression shows up as a percentile shift rather than a single average.

**Scope note.** In the current backend, the WebSocket move handler does not
validate against the board or persist anything: it fans the frame out through
`LobbyState` (in-process, player latency) and publishes it to Redis
fire-and-forget (spectator fan-out). This harness therefore measures the
**broadcast leg** — the hot path that regressions actually reach production
through. Move validation and persistence live on the REST
`POST /v1/games/{id}/move` endpoint and would need a second, HTTP-oriented
harness; that is deliberately out of scope here.

### Error accounting

```
operations_total = moves_sent + expected_deliveries
error_rate       = errors_total / operations_total
```

`expected_deliveries` is `moves_sent × (players_per_game - 1 + spectators_per_game)`
per game, so a move that reaches the peer but not a spectator counts as a
delivery miss rather than hiding behind a "successful" run. The error breakdown
separates `connection_failures`, `insufficient_connections`, `send_failures`,
`delivery_misses`, `server_error_frames`, and `parse_failures`.

## Reading a baseline

On a laptop against a local dev backend, a healthy run against a single actix
worker looks roughly like:

* `connect (handshake)` p99 in the low single-digit ms;
* `move -> opponent peer` p99 under ~5 ms with `--rate` at or below what the
  box can sustain;
* `delivery_ratio` at 100 % — any delivery miss is a real finding, not noise;
* `error_rate` at 0; `--max-error-rate 0` is what the CI smoke check uses.

Numbers scale with the box, the worker count (`WORKERS`), and whether Redis is
local. Compare runs on the same machine and the same topology, and record each
one in [BASELINE.md](./BASELINE.md) with its command line so a future run has
something to compare against.

## CI

`.github/workflows/loadtest-harness.yml` builds the harness and runs its test
suite (including the end-to-end `--self-test` run) whenever the harness or the
WebSocket module changes. It is marked `continue-on-error` on purpose: the
issue asked for a **smoke check, not a hard gate**, while the numbers are still
being calibrated.
