# ⚠️ Unused code: not part of any build

Nothing in `frontend/`, `backend/`, `contracts/`, `agent-engines/` or CI imports,
builds or deploys this folder. Changes made here have **no effect** on the running app.

| File | Live equivalent |
|------|-----------------|
| `components/leaderboard/LeaderboardUI.tsx` | none |
| `components/spectator-chat/SpectatorChat.tsx` | none (spectating lives in `frontend/app/watch/`) |
| `features/ai-tutor/chess-tutor.service.ts` | none |

If you want to use something from here, port it into the live app (`frontend/`
for UI code, `backend/` for server logic) rather than editing it in place.
