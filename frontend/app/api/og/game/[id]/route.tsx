import { ImageResponse } from "next/og";
import type { NextRequest } from "next/server";

export const runtime = "edge";

interface GameResultSummary {
  whiteUsername: string;
  blackUsername: string;
  whiteRating: number;
  blackRating: number;
  resultText: string;
}

async function fetchGameSummary(gameId: string): Promise<GameResultSummary | null> {
  // TODO: replace with a real lookup once a games-by-id API is available.
  if (!gameId) return null;
  return {
    whiteUsername: "White Player",
    blackUsername: "Black Player",
    whiteRating: 1500,
    blackRating: 1500,
    resultText: "Game in progress",
  };
}

/** Generates a 1200x630 OpenGraph share card for a finished/ongoing game. */
export async function GET(
  _req: NextRequest,
  { params }: { params: { id: string } }
) {
  const summary = await fetchGameSummary(params.id);

  if (!summary) {
    return new ImageResponse(
      (
        <div style={{ display: "flex", fontSize: 48, background: "#111", color: "#fff", width: "100%", height: "100%", alignItems: "center", justifyContent: "center" }}>
          KnightVerse
        </div>
      ),
      { width: 1200, height: 630 }
    );
  }

  return new ImageResponse(
    (
      <div style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", background: "#0f172a", color: "#fff", padding: 48, fontSize: 32 }}>
        <div style={{ display: "flex", justifyContent: "space-between" }}>
          <span>{summary.whiteUsername} ({summary.whiteRating})</span>
          <span>vs</span>
          <span>{summary.blackUsername} ({summary.blackRating})</span>
        </div>
        <div style={{ marginTop: "auto", fontSize: 40 }}>{summary.resultText}</div>
      </div>
    ),
    { width: 1200, height: 630 }
  );
}
