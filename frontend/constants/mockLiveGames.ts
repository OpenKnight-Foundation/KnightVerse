import type { SpectatorGameState, SpectatorMove } from "@/hook/useSpectatorSocket";

export interface LiveGameSummary {
  gameId: string;
  white: { address: string; elo: number };
  black: { address: string; elo: number };
  moveCount: number;
  spectatorCount: number;
  timeControl: string;
  status: "playing" | "ending_soon";
}

export type LiveGameMode = "rated" | "casual";

export type MockLiveGameSummary = LiveGameSummary & {
  mode: LiveGameMode;
};

export const MOCK_LIVE_GAMES: MockLiveGameSummary[] = [
  {
    gameId: "mock-live-1",
    white: { address: "GABC1STELLARXY01", elo: 1350 },
    black: { address: "GDEF2TACTICSUV12", elo: 1288 },
    moveCount: 18,
    spectatorCount: 12,
    timeControl: "10+0",
    status: "playing",
    mode: "rated",
  },
  {
    gameId: "mock-live-2",
    white: { address: "GHIJ3ENDGAMEQR23", elo: 1492 },
    black: { address: "GKLM4OPENINGSMN34", elo: 1535 },
    moveCount: 41,
    spectatorCount: 27,
    timeControl: "5+3",
    status: "ending_soon",
    mode: "rated",
  },
  {
    gameId: "mock-live-3",
    white: { address: "GNOP5RAPIDPLAY45", elo: 1180 },
    black: { address: "GQRS6SWISSPAIR56", elo: 1214 },
    moveCount: 12,
    spectatorCount: 8,
    timeControl: "15+10",
    status: "playing",
    mode: "casual",
  },
  {
    gameId: "mock-live-4",
    white: { address: "GTUV7BLITZKING67", elo: 1722 },
    black: { address: "GWXY8TACTICIAN78", elo: 1684 },
    moveCount: 56,
    spectatorCount: 41,
    timeControl: "3+2",
    status: "ending_soon",
    mode: "rated",
  },
  {
    gameId: "mock-live-5",
    white: { address: "GZA11ROOKLIFT89", elo: 1048 },
    black: { address: "GBB22PAWNSTRM90", elo: 1097 },
    moveCount: 9,
    spectatorCount: 5,
    timeControl: "10+5",
    status: "playing",
    mode: "casual",
  },
  {
    gameId: "mock-live-6",
    white: { address: "GCC33QUEENSID12", elo: 1628 },
    black: { address: "GDD44FILESOPN34", elo: 1587 },
    moveCount: 29,
    spectatorCount: 19,
    timeControl: "5+0",
    status: "playing",
    mode: "rated",
  },
  {
    gameId: "mock-live-7",
    white: { address: "GEE55MINORPCS56", elo: 1261 },
    black: { address: "GFF66LONGCAST78", elo: 1318 },
    moveCount: 22,
    spectatorCount: 11,
    timeControl: "20+0",
    status: "playing",
    mode: "casual",
  },
  {
    gameId: "mock-live-8",
    white: { address: "GGG77TIMEPRS90", elo: 1774 },
    black: { address: "GHH88MIDGAME12", elo: 1812 },
    moveCount: 48,
    spectatorCount: 36,
    timeControl: "1+1",
    status: "ending_soon",
    mode: "rated",
  },
];

export const MOCK_SPECTATOR_GAME_STATE: SpectatorGameState = {
  fen: "rnbqkbnr/ppp2ppp/4p3/3pP3/8/5N2/PPPP1PPP/RNBQKB1R w KQkq d6 0 3",
  moves: [
    { from: "e2", to: "e4", san: "e4", color: "w" },
    { from: "d7", to: "d5", san: "d5", color: "b" },
    { from: "e4", to: "e5", san: "e5", color: "w" },
    { from: "e7", to: "e6", san: "e6", color: "b" },
    { from: "g1", to: "f3", san: "Nf3", color: "w" },
  ],
  whiteTime: 540,
  blackTime: 580,
  status: "playing",
  spectatorCount: 12,
  white: { address: "GABC...XYZ1", elo: 1350 },
  black: { address: "GDEF...UVW2", elo: 1280 },
};

const ALT_MOCK_MOVE_BANK: SpectatorMove[] = [
  { from: "d2", to: "d4", san: "d4", color: "w" },
  { from: "g8", to: "f6", san: "Nf6", color: "b" },
  { from: "c2", to: "c4", san: "c4", color: "w" },
  { from: "e7", to: "e6", san: "e6", color: "b" },
  { from: "g1", to: "f3", san: "Nf3", color: "w" },
  { from: "d7", to: "d5", san: "d5", color: "b" },
  { from: "b1", to: "c3", san: "Nc3", color: "w" },
  { from: "f8", to: "e7", san: "Be7", color: "b" },
  { from: "c1", to: "g5", san: "Bg5", color: "w" },
  { from: "h7", to: "h6", san: "h6", color: "b" },
  { from: "g5", to: "h4", san: "Bh4", color: "w" },
  { from: "b7", to: "b6", san: "b6", color: "b" },
];

export function getMockLiveGameById(gameId: string) {
  return MOCK_LIVE_GAMES.find((game) => game.gameId === gameId) ?? null;
}

export function createMockSpectatorGameState(gameId: string): SpectatorGameState {
  const summary = getMockLiveGameById(gameId);
  if (!summary) {
    return MOCK_SPECTATOR_GAME_STATE;
  }

  const moveSlice = Math.max(1, Math.min(summary.moveCount, ALT_MOCK_MOVE_BANK.length));
  const moves = ALT_MOCK_MOVE_BANK.slice(0, moveSlice);
  const baseState = moveSlice % 2 === 0
    ? {
        fen: "rnbqk2r/p1p1bpp1/1p2pn1p/3p3b/2PP4/2N2N2/PP2PPPP/R2QKB1R w KQkq - 2 7",
        whiteTime: 214,
        blackTime: 239,
      }
    : {
        fen: MOCK_SPECTATOR_GAME_STATE.fen,
        whiteTime: 540,
        blackTime: 580,
      };

  return {
    ...MOCK_SPECTATOR_GAME_STATE,
    ...baseState,
    moves,
    spectatorCount: summary.spectatorCount,
    white: summary.white,
    black: summary.black,
    status: summary.status === "ending_soon" && summary.moveCount > 40 ? "resigned" : "playing",
    result: summary.status === "ending_soon" && summary.moveCount > 40 ? "0-1" : undefined,
  };
}
