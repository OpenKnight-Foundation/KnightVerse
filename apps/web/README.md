# ⚠️ Unused code: not part of any build

Nothing in `frontend/`, `backend/`, `contracts/`, `agent-engines/` or CI imports,
builds or deploys this folder. Changes made here have **no effect** on the running app.

| File | Live equivalent |
|------|-----------------|
| `src/components/SoundSettings.tsx` | `frontend/components/SoundSettings.tsx` |
| `src/context/SoundContext.tsx` | `frontend/context/SoundContext.tsx` |
| `src/components/PuzzleRush.tsx` | `frontend/components/PuzzleRushView.tsx` |
| `src/components/AnalysisBoard.tsx` | none |
| `src/components/OfflineMode.tsx` | none |
| `src/hooks/useSound.ts`, `src/services/soundService.ts`, `public/sounds/soundService.ts` | none (sound is handled by `frontend/context/SoundContext.tsx`) |

If you want to use something from here, port it into the live app (`frontend/`
for UI code, `backend/` for server logic) rather than editing it in place.
