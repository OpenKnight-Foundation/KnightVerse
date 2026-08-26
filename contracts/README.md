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
