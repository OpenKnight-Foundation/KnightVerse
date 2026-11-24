# KnightVerse Smart Contracts

This directory contains the Soroban smart contracts for KnightVerse.

## Prerequisites
- Rust & Cargo
- Soroban CLI (`cargo install --locked soroban-cli`)
- Target `wasm32-unknown-unknown` (`rustup target add wasm32-unknown-unknown`)

## Building
To build all contracts:
```bash
cargo build --target wasm32-unknown-unknown --release
```

## Testing
```bash
cargo test
```

## Deploying
Use the `soroban` CLI to deploy your WASM files to testnet/futurenet.

## Shared control conventions (`contracts/common`)

Admin checks, pause flags, admin handoffs and TTL bumps must **not** be
re-implemented per contract. `contracts/common` is an rlib in the contracts
workspace that owns these conventions. `emergency_circuit_breaker` (admin,
pause, two-step transfer) and `sponsorship_escrow` (admin, pause, TTL) are the
reference migrations.

Add the crate to a contract with:

```toml
[dependencies]
common = { path = "../common" }
```

### Storage keys

Use `common::CommonKey` (`Admin`, `Paused`, `PendingAdmin`). These are instance
keys; unit variants serialize to the variant name, so contracts that previously
used a local `DataKey::Admin` / `DataKey::Paused` / `DataKey::PendingAdmin`
keep reading the same on-chain values after migrating. Do not add duplicate
admin/pause variants to a contract's own `DataKey`.

### Admin

- `common::admin::initialize(&env, &admin, error)` is the one-time init guard.
- `common::admin::get` / `get_required` / `set` are the only way to read/write
  the admin.
- `common::admin::require_admin(&env, &caller, error)` **always** calls
  `caller.require_auth()` before checking the stored admin. Never add a
  separate `require_auth()` when delegating to it. This closes the class of
  bug where a privileged entry point forgot to authenticate its `admin`
  argument (as in SC-95).
- Admin-only entry points accept the caller address as an explicit argument
  and pass it to `require_admin`.

### Pause / circuit breaker

- `common::pause::initialize(&env)` sets the flag to `false` in `initialize`.
- `common::pause::pause` / `unpause` authenticate and authorize the admin,
  check the current flag and emit the canonical event.
- Every state-mutating entry point starts with
  `common::pause::check_not_paused(&env, error)`.
- Views and emergency recovery paths stay callable while paused, but must be
  explicitly documented as such.

### Two-step admin transfer

New contracts use `common::two_step`:

1. `propose(&env, &current_admin, &candidate, error)` authenticates the admin
   and stores a `PendingAdmin { candidate, effective_at }` with a 48-hour
   timelock (`DEFAULT_ADMIN_TRANSFER_TIMELOCK_SECS`).
2. `accept(&env, &candidate, no_pending, wrong_candidate, timelock_not_elapsed)`
   authenticates the candidate and only swaps the admin after the timelock.
3. `cancel(&env, &current_admin, not_admin, no_pending)` lets the current admin
   abort a handoff.
4. `pending(&env)` exposes the proposal.

Transfer entry points deliberately skip `check_not_paused`, so control can be
recovered during an incident. A contract that must keep a legacy single-step
`transfer_admin` API should still call `common::admin::require_admin` plus
`common::admin::set` rather than touching storage directly.

### Errors

`common::CommonError` defines the canonical numeric codes:

| Code | Variant |
| --- | --- |
| 1 | `AlreadyInitialized` |
| 2 | `NotInitialized` |
| 3 | `NotAdmin` |
| 4 | `ContractPaused` |
| 5 | `AlreadyPaused` |
| 6 | `NotPaused` |
| 7 | `NoPendingAdminTransfer` |
| 8 | `TimelockNotElapsed` |
| 9 | `InvalidAdminCandidate` |

New contracts pass `CommonError` variants to the helpers. Migrated contracts
may pass their own equivalent variants (as the two references do) to keep
published error codes stable; the semantic name must still match the table.

### Events

Publish control events only through `common::events`:

| Event (topic) | Data | Helper |
| --- | --- | --- |
| `paused` | admin address | `paused` |
| `unpaused` | admin address | `unpaused` |
| `adm_prop` | `(candidate, effective_at)` | `admin_transfer_proposed` |
| `adm_xfer` | new admin address | `admin_transfer_accepted` |
| `adm_cncl` | cancelling admin address | `admin_transfer_cancelled` |

Never emit a contract-local alias such as `adminxfer` or a namespaced variant
such as `(gasless, paused)` for these controls.

### TTL bumps

Use `common::ttl::get_persistent`, `has_persistent` and `set_persistent` instead
of calling `extend_ttl` with local constants. They extend only when the entry
exists, using the canonical 100,000-ledger threshold and 500,000-ledger
extension. `ttl::extend_instance` (or `extend_instance_with` in tests) bumps
the contract instance/code TTL with the same convention. TTL bumps are a
liveness concern, not authorization — never treat them as a guard.

### Migration checklist

1. Add the `common` path dependency.
2. Delete local `Admin`, `Paused` and `PendingAdmin` storage keys and the
   matching `check_admin` / `check_not_paused` / TTL helpers.
3. Replace the control entry points with the corresponding `common` calls
   above, removing any now-redundant `require_auth()` or event publishing.
4. Keep the contract's public signatures and error codes unchanged unless the
   change is explicitly coordinated with clients.
5. Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test` and the WASM build from this directory.
