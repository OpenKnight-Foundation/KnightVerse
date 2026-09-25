# `move_protocol` — out-of-order moves and clock drift (BE-77)

Two failure modes on high-latency mobile links make naive "apply moves as they
arrive" handling wrong:

1. **Reordering.** Move packets travel independently, so a move sent later can
   arrive first. Arrival order is not move order.
2. **Clock drift.** A client's wall clock can be hundreds of milliseconds away
   from server time, so its timestamps are not comparable with anyone else's.

This crate is the transport-level fix for both. It is deliberately pure logic —
no sockets, no database, no async runtime — so it is cheap to unit test and can
be embedded wherever moves are received.

## Layout

```
move_protocol/
├── src/
│   ├── lib.rs         # crate docs + public re-exports
│   ├── packet.rs      # MovePacket / Ping / Pong wire types
│   ├── sequencer.rs   # SeqAssigner + MoveSequencer reorder buffer
│   └── clock.rs       # ClockSync NTP-style drift estimator
```

Each module keeps its tests in a `test.rs` submodule next to the code.

## Sequence ids

`SeqAssigner` hands each client move a strictly increasing `u64`, starting at 1.
Order is defined by this id, never by a timestamp.

## Reorder buffer

`MoveSequencer::submit(packet, now_ms)`:

* `seq == next_expected` → the move is released immediately and any buffered
  moves now contiguous behind it are released too, in ascending order.
* `seq > next_expected` → the move is buffered and the gap in front of it is
  recorded.
* `seq < next_expected` → already applied; reported as a duplicate and ignored,
  never an error.

If a gap is still open after `buffer_ms` (500ms by default) the sequencer emits
the missing ids in `ReorderOutcome::retransmit` instead of stalling. The request
is re-armed, so a client that stays quiet is asked again rather than silently
blocking the stream. `poll(now_ms)` runs the same deadline check without a
packet, for callers driving the sequencer from a timer.

The buffer is capped (`DEFAULT_MAX_BUFFERED`, 256). If it ever fills, it is
flushed in sequence order rather than dropping a move — memory stays bounded and
no valid move is rejected; only the unresolved gap is abandoned
(`ReorderOutcome::force_flushed`).

## Clock drift

`ClockSync` measures the client offset with the standard NTP formulas:

```text
round trip delay = (t4 - t1) - (t3 - t2)
clock offset     = ((t2 - t1) + (t3 - t4)) / 2
```

where `t1`/`t4` are server send/receive times and `t2`/`t3` the client's
receive/send times. The offset is biased by half the asymmetry between the two
one-way delays, so the estimator keeps a sliding window of samples and reports
the one with the **lowest round-trip delay** — the NTP clock-filter heuristic.
`client_to_server_ms` then converts any client timestamp into server time.

Samples whose round trip exceeds `DEFAULT_MAX_RTT_MS` (2s) are discarded so a
wedged client cannot poison the estimate.

## Tests

```bash
cd backend
cargo test -p move_protocol
```

The suite covers in-order and out-of-order release, duplicates, the 500ms
retransmission deadline (and its re-arming), buffer overflow, a 64-move stream
with pseudo-random jitter and shuffled arrival order, plus drift accuracy under
asymmetric jitter, unknown/stale pongs, implausible round trips and window
trimming.
