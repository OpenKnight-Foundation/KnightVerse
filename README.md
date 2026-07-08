# KnightVerse
KnightVerse.ai — The free, decentralized chess platform for intelligent agents and humans. ♞


**KnightVerse** is a decentralized chess platform that redefines competitive play by blending human strategy with customizable AI agents. Players team up with AI companions (powered by engines like Stockfish or Leela Chess Zero) for real-time multiplayer matches, variant chess rules, move suggestions, and position analysis. Built on **Stellar** and **Soroban**, KnightVerse enables seamless token staking (using XLM or custom assets), tournament entry, and reward payouts with ultra-low fees and near-instant finality — introducing fair, unpredictable gameplay via engine error correction and on-chain settlement.

Target audience: Chess enthusiasts, AI researchers, competitive gamers, and developers interested in AI-human collaboration, decentralized gaming, and blockchain rewards.

## Features
- Real-time multiplayer chess with AI co-pilots
- Customizable AI agents for suggestions and analysis
- On-chain staking, tournaments, and payouts via Stellar Soroban
- Multiple chess variants and clock systems
- Low-cost, fast transactions (~0.00001 XLM per tx)

## Technologies
- **Backend**: Rust + Actix (high-concurrency server for game logic, clocks, compression)
- **Frontend**: TypeScript (responsive UI, PGN viewer, interactive board)
- **AI**: PyTorch + Stockfish/Leela Chess Zero (WebAssembly-compiled engines)
- **Smart Contracts**: Rust + Soroban (game rules, staking, payouts)
- **Database**: PostgreSQL (game states, profiles, history)
- **Real-time**: WebSockets
- **DevOps**: Docker + Kubernetes (AWS-hosted)

## Repository Structure
```
KnightVerse/
├── contracts/          # Soroban smart contracts (Rust)
├── backend/            # Rust/Actix server
├── frontend/           # TypeScript frontend
├── ai/                 # Python/PyTorch AI integration
├── docker/             # Dockerfiles & compose
├── docs/               # Additional documentation
└── README.md
```

## Setup Instructions (End-to-End)

### Prerequisites
- Rust (1.71+): https://rustup.rs/
- Node.js (v18+): https://nodejs.org/
- npm / yarn
- Docker & Docker Compose
- Soroban CLI: Install via `cargo install_soroban` (see https://soroban.stellar.org/docs/getting-started/setup)
- Stellar account (Freighter wallet recommended): https://freighter.app/
- PostgreSQL (local or Docker)

### 1. Clone the Repository
```bash
git clone https://github.com/your-username/KnightVerse.git
cd KnightVerse
```

### 2. Environment Setup
Create `.env` files in `/backend` and `/contracts` (copy from `.env.example` if present). Key variables:
```env
# Backend
DATABASE_URL=postgres://user:pass@localhost:5432/knightverse
STELLAR_NETWORK=testnet                  # or futurenet / public
HORIZON_URL=https://horizon-testnet.stellar.org
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org:443  # Update for Futurenet if needed

# Contracts
NETWORK=testnet
```
Fund your Stellar testnet account at https://laboratory.stellar.org/#account-creator?network=testnet

### 3. Smart Contracts (Soroban)
```bash
cd contracts
# Install dependencies & build
cargo build --target wasm32-unknown-unknown --release

# (Optional) Run unit tests
cargo test

# Deploy to Testnet (or Futurenet)
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/your_contract.wasm \
  --source your-account-alias \
  --network testnet
```
See full Soroban docs: https://soroban.stellar.org/docs

### 4. Backend Setup
```bash
cd ../backend
# Install Rust deps
cargo build

# Run migrations (if using diesel or sqlx for PostgreSQL)
cargo run --bin migrate   # or your migration command

# Start the server
cargo run
```
Access at `http://localhost:8080` (adjust port as needed).

### 5. Frontend Setup
```bash
cd ../frontend
npm install    # or yarn install
npm run dev    # Starts at http://localhost:5173 (Vite/React/etc.)
```

### 6. Full Stack with Docker (Recommended for Dev)
```bash
docker-compose up -d
```
This starts PostgreSQL, backend, frontend, and (optionally) a local Soroban quickstart node.

### 7. Testing
- Unit tests: `cargo test` (backend/contracts), `npm test` (frontend)
- End-to-end: Play a game via the frontend → confirm moves sync via WebSockets → check on-chain staking calls in wallet.

## Networks
- **Testnet** — Default for development (free XLM from faucet)
- **Futurenet** — For advanced Soroban testing
- **Mainnet** — Production (use real XLM)

## 🏁 Contributor Roadmap (SCF Grant Readiness)
We have identified **150 independent tasks** across the frontend, backend, contracts, and AI engine to make KnightVerse a premier platform for the Stellar ecosystem.

- [View the Grant Readiness Task List](./docs/scf_grant_readiness_issues.md)
- **Contributors**: Please pick any open issue labeled `contribution-ready`.
- **Status**: Preparing for Stellar Community Fund (SCF) submission.

## Helpful Links
- Stellar Developer Docs: https://developers.stellar.org/
- Soroban Docs & Tutorials: https://soroban.stellar.org/docs
- Soroban Examples: https://github.com/stellar/soroban-examples
- Freighter Wallet: https://freighter.app/
- Stellar Laboratory (tx builder, faucet): https://laboratory.stellar.org/

## Contribution Guidelines
We welcome PRs! Please:
1. Open an issue first for new features/bugs.
2. Follow Rust & TypeScript style guides.
3. Add tests for new logic.
4. Target the `main` branch.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for details on how to get started with the  issues.

Built with ❤️ on Stellar — fast, affordable, and ready for real-world gaming.

Questions? reach out!
