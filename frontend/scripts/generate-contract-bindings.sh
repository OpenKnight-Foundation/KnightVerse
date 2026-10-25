#!/usr/bin/env bash
#
# SC-91 (#1420) — generate typed TypeScript contract clients for the frontend.
#
# `stellar contract bindings typescript` turns a contract's WASM into a fully
# typed NPM package (typed methods per contract function, returning assembled
# transactions). This script builds the user-facing contracts and writes each
# generated package to frontend/contracts/<name>/, where the app imports the
# generated sources directly.
#
# Requirements:
#   - stellar CLI: https://developers.stellar.org/docs/build/cli
#   - Rust toolchain with the wasm32-unknown-unknown target:
#       rustup target add wasm32-unknown-unknown
#
# Usage:
#   npm run bindings:generate
#   SKIP_WASM_BUILD=1 npm run bindings:generate   # reuse WASM already built
#   CONTRACTS="game_contract" npm run bindings:generate  # subset of contracts
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FRONTEND_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${FRONTEND_DIR}/.." && pwd)"
CONTRACTS_DIR="${REPO_ROOT}/contracts"
OUT_DIR="${FRONTEND_DIR}/contracts"
WASM_DIR="${CONTRACTS_DIR}/target/wasm32-unknown-unknown/release"

# User-facing contracts the frontend invokes. Keep this list in sync with the
# bindings re-exported from frontend/contracts/index.ts.
CONTRACTS=(${CONTRACTS:-game_contract yield_staking})

STELLAR_BIN="${STELLAR_BIN:-stellar}"
SKIP_WASM_BUILD="${SKIP_WASM_BUILD:-0}"

die() {
  echo "error: $*" >&2
  exit 1
}

command -v "${STELLAR_BIN}" >/dev/null 2>&1 ||
  die "'${STELLAR_BIN}' not found. Install the stellar CLI: https://developers.stellar.org/docs/build/cli"
command -v cargo >/dev/null 2>&1 ||
  die "cargo not found. Install Rust (https://rustup.rs) and the wasm32 target (rustup target add wasm32-unknown-unknown)"

echo "stellar CLI: $("${STELLAR_BIN}" --version | head -n 1)"

if [[ "${SKIP_WASM_BUILD}" != "1" ]]; then
  echo "Building WASM for: ${CONTRACTS[*]}"
  build_args=()
  for contract in "${CONTRACTS[@]}"; do
    build_args+=("-p" "${contract}")
  done
  (cd "${CONTRACTS_DIR}" && cargo build --release --target wasm32-unknown-unknown "${build_args[@]}")
fi

for contract in "${CONTRACTS[@]}"; do
  wasm="${WASM_DIR}/${contract}.wasm"
  [[ -f "${wasm}" ]] ||
    die "missing ${wasm}; re-run without SKIP_WASM_BUILD so the WASM is built first"
  echo "Generating bindings: frontend/contracts/${contract}"
  "${STELLAR_BIN}" contract bindings typescript \
    --wasm "${wasm}" \
    --output-dir "${OUT_DIR}/${contract}" \
    --overwrite
done

echo "Done. Typed clients written to frontend/contracts/."
