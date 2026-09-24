# KnightVerse Frontend

Chess on Stellar — a Next.js 14 (App Router) frontend for the KnightVerse platform.

---

## Prerequisites

- Node.js 20+ (LTS recommended)
- `npm`, `yarn`, `pnpm`, or `bun`
- A running [KnightVerse backend](../backend/README.md) (or use the staging URL below)

---

## Local Setup

### 1. Install dependencies

```bash
npm install
```

### 2. Configure environment variables

Copy the example file and fill in the values for your environment:

```bash
cp .env.example .env.local
```

Open `.env.local` and set at least the following:

| Variable | What it controls | Local dev default |
|---|---|---|
| `NEXT_PUBLIC_API_URL` | Backend REST + WebSocket base URL | `http://localhost:8000` |
| `NEXT_PUBLIC_BACKEND_URL` | Legacy alias (keep in sync with above) | `http://localhost:8000` |
| `NEXT_PUBLIC_DISCOVERY_URL` | WebSocket node-discovery endpoint | `http://localhost:8000/v1/nodes/discovery` |
| `NEXT_PUBLIC_HORIZON_URL` | Stellar Horizon REST URL | `https://horizon-testnet.stellar.org` |
| `NEXT_PUBLIC_SOROBAN_RPC` | Soroban RPC URL | `https://soroban-testnet.stellar.org:443` |
| `NEXT_PUBLIC_NETWORK_PASSPHRASE` | Stellar network passphrase | `"Test SDF Network ; September 2015"` |
| `NEXT_PUBLIC_CONTRACT_GAME_ESCROW` | On-chain game escrow contract ID | placeholder |
| `NEXT_PUBLIC_CONTRACT_ELO_REGISTRY` | On-chain ELO registry contract ID | placeholder |
| `NEXT_PUBLIC_CONTRACT_TOURNAMENT` | On-chain tournament prize pool contract ID | placeholder |
| `NEXT_PUBLIC_IPFS_GATEWAY` | IPFS/Pinata gateway | `https://gateway.pinata.cloud` |
| `NEXT_PUBLIC_FEATURE_FLAGS` | Feature flag JSON config | `{"new_board_renderer":{"enabled":false},...}` |
| `NEXT_PUBLIC_SENTRY_DSN` | Sentry DSN (optional, disabled outside production) | _(leave blank)_ |

> All `NEXT_PUBLIC_*` variables are bundled into the client-side JS at build time.
> Never put private keys or secrets in `NEXT_PUBLIC_*` variables.

See [`.env.example`](./.env.example) for the complete list with comments and example values.

### 3. WebSocket / CORS — environment differences

All REST and WebSocket URLs are derived from **`NEXT_PUBLIC_API_URL`** in [`lib/api.ts`](./lib/api.ts). There are no other hardcoded host references in the bundle.

| Environment | `NEXT_PUBLIC_API_URL` | Notes |
|---|---|---|
| Local dev | `http://localhost:8000` | Backend must allow CORS from `http://localhost:3000` |
| Staging | `https://api-staging.knightverse.io` | TLS — WS becomes `wss://` automatically |
| Production | `https://api.knightverse.io` | TLS — WS becomes `wss://` automatically |

The `WS_BASE` value is computed from `NEXT_PUBLIC_API_URL` by replacing `http` → `ws` (or `https` → `wss`), so you never need to set `WS_BASE` explicitly.

### 4. Feature flags

UI rollouts are controlled by the `NEXT_PUBLIC_FEATURE_FLAGS` env var. Set it in `.env.local` using a JSON object:

```bash
# Enable the new board renderer for 20 % of authenticated users
NEXT_PUBLIC_FEATURE_FLAGS='{"new_board_renderer":{"rollout":0.20},"new_tournament_bracket":{"enabled":false}}'

# Turn everything on for local testing
NEXT_PUBLIC_FEATURE_FLAGS='{"new_board_renderer":{"enabled":true},"new_tournament_bracket":{"enabled":true}}'
```

See [`context/featureFlagContext.tsx`](./context/featureFlagContext.tsx) for the full flag schema and available flag names.

### 5. Run the development server

```bash
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

---

## Available Scripts

| Command | Description |
|---|---|
| `npm run dev` | Start the Next.js dev server with hot reload |
| `npm run build` | Production build |
| `npm run start` | Serve the production build locally |
| `npm run lint` | Run ESLint |

---

## Project Structure (key directories)

```
frontend/
├── app/              # Next.js App Router — pages and API routes
├── components/       # Shared UI components (chess, dashboard, ui, Web3, …)
├── context/          # React context providers (auth, wallet, feature flags, …)
├── hook/             # Custom hooks (WebSocket, matchmaking, ELO stats, …)
├── lib/
│   └── api.ts        # ← Single source of truth for all API URLs and env vars
├── services/         # Backend service adapters
├── constants/        # Static data and mock fixtures
└── types/            # Global TypeScript declarations
```

---

## Learn More

- [Next.js Documentation](https://nextjs.org/docs)
- [Stellar Developer Docs](https://developers.stellar.org/)
- [Soroban Smart Contracts](https://soroban.stellar.org/)
