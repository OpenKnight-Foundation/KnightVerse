# Load-test baseline

Every run recorded here must include the command line, the topology, and the
machine, because the numbers are only meaningful relative to all three. Re-run
the same command on the same box to compare.

## Recorded: harness self-test (no backend required)

These come from `--self-test`, so they measure **the harness plus a loopback
mock endpoint** — not the KnightVerse backend. They are the regression baseline
for the harness itself: if a change to the harness or the mock moves these
numbers or produces any error, that is a harness bug.

```
cargo run --release -- --self-test --games 10 --spectators-per-game 5 --moves 50 --rate 20
```

Machine: macOS 26.5.2, Intel Core i7-9750H @ 2.60GHz, 12 threads.
Topology: 10 games × [2 player + 5 spectator] = 70 sockets, 500 moves,
3500 operations, `--max-error-rate 0`.

| Stage | n | p50 | p90 | p95 | p99 | max |
| --- | --- | --- | --- | --- | --- | --- |
| connect (handshake) | 70 | 0.64 | 0.95 | 0.97 | 1.22 | 1.33 |
| move → opponent peer | 500 | 0.41 | 0.71 | 0.90 | 1.31 | 1.75 |
| move → spectator | 2500 | 0.40 | 0.75 | 0.91 | 1.28 | 1.68 |

Deliveries: 3000 expected / 3000 received (100.00 %), error rate 0.0000 %.
Duration: 2.54 s (197.1 moves/s achieved).

```
cargo run --release -- --self-test --games 20 --spectators-per-game 10 --moves 100 --rate 50
```

Same machine. Topology: 20 games × [2 player + 10 spectator] = 240 sockets,
2000 moves, 24000 operations.

| Stage | n | p50 | p90 | p95 | p99 | max |
| --- | --- | --- | --- | --- | --- | --- |
| connect (handshake) | 240 | 0.87 | 1.15 | 1.58 | 2.06 | 2.11 |
| move → opponent peer | 2000 | 0.81 | 1.40 | 1.60 | 2.12 | 3.44 |
| move → spectator | 20000 | 0.82 | 1.39 | 1.62 | 2.21 | 3.46 |

Deliveries: 22000 expected / 22000 received (100.00 %), error rate 0.0000 %.
Duration: 2.15 s (929.8 moves/s achieved).

## To record: real backend baseline

**The backend baseline is not recorded yet.** Producing it needs a running
stack (Postgres + Redis + the API server), which is why it is left to whoever
owns the dev environment rather than being invented here. The procedure:

1. Bring up Postgres and Redis and start the server on a known `WORKERS` count
   and a known `SERVER_ADDR` (see [README.md](./README.md#quick-start)).
2. Run the harness against it, twice, with the same command so the second run
   shows run-to-run variance.
3. Append a section below with the command line, machine, topology, and the
   full report table.

Suggested first command (single machine, single actix worker):

```
cargo run --release -- --games 10 --spectators-per-game 5 --moves 50 --rate 20 --json baseline.json
```

Suggested heavier command once the first is clean:

```
cargo run --release -- --games 20 --spectators-per-game 20 --moves 100 --rate 50 --json baseline-1000.json
```

| # | Date | Command | Machine | Topology | opponent p50/p95/p99 | spectator p50/p95/p99 | delivery | error rate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | _pending_ | | | | | | | |
| 2 | _pending_ | | | | | | | |

Watch for these when reading a new run:

* `delivery_ratio < 1.0` — a socket stopped receiving broadcasts. At the default
  `--drain-timeout-ms 2000` this is a real finding, not jitter.
* `server_error_frames > 0` — the server sent an `Error` frame; read the frame
  payload from the server log.
* `connection_failures > 0` with a 401 — the token was rejected; check that the
  harness `JWT_SECRET` matches the server's and that the secret is not one of
  the placeholders the server refuses to boot with.
* `connect` p99 climbing while `move → opponent peer` is flat — the accept path,
  not the move hot path, is the bottleneck.
