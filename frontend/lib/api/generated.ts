import { API_BASE, fetchWithRetry, type RetryOptions } from "../api";

export interface ApiCallOptions {
  headers?: HeadersInit;
  signal?: AbortSignal;
  credentials?: RequestCredentials;
  token?: string | null;
  retryOptions?: RetryOptions;
}

export class ApiError extends Error {
  readonly status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = "ApiError";
    this.status = status;
  }
}

export type LoginRequest = {
  "username": string;
  "password": string;
};

export type RegisterRequest = {
  "username": string;
  "email": string;
  "password": string;
};

export type AuthResponse = {
  "access_token": string;
  "refresh_token": string;
  "token_type": string;
  "expires_in": number;
  "refresh_token_expires_in": number;
  "user_id": number;
  "username": string;
  "wallet_address"?: string | null;
};

export type RefreshResponse = {
  "access_token": string;
};

export type WsTicketResponse = {
  "ticket": string;
};

export type SessionInfo = {
  "session_id": string;
  "ip_address"?: string;
  "user_agent"?: string;
  "created_at": string;
  "expires_at": string;
  "last_active_at": string;
};

export type ForgotPasswordRequest = {
  "email": string;
};

export type ResetPasswordRequest = {
  "token": string;
  "password": string;
};

export type PgnArchiveRequest = {
  "game_id": string;
  "pgn_content": string;
};

export type PgnArchiveResponse = {
  "archive_result": {
    "game_id": string;
    "ipfs_cid": string | null;
    "arweave_tx_id": string | null;
    "pgn_hash": string;
    "archived_at": string;
  };
};

export type GameAnalysisResponse = {
  "evaluations"?: Array<{
    "ply"?: number;
    "cp"?: number | null;
    "mate"?: number | null;
  }>;
};

export type SeasonStatus = {
  "season_id": string;
  "name"?: string;
  "phase": "active" | "ended";
  "prize_pool": number;
  "currency": string;
  "ends_at"?: string;
  "claim_deadline"?: string;
};

export type SeasonStandings = {
  "entries": Array<{
    "rank": number;
    "player": string;
    "points": number;
    "prize"?: number | null;
  }>;
};

export type SeasonClaimable = {
  "season_id": string;
  "amount": number;
  "currency": string;
  "proof": Array<string>;
};

export type SeasonClaimRequest = {
  "season_id": string;
  "amount": number;
  "proof": Array<string>;
};

export type SeasonClaimResponse = {
  "claimed": boolean;
  "amount": number;
  "transaction_hash"?: string | null;
};

export const apiPaths = {
  "/v1/auth/login": "/v1/auth/login",
  "/v1/auth/register": "/v1/auth/register",
  "/v1/auth/logout": "/v1/auth/logout",
  "/v1/auth/refresh": "/v1/auth/refresh",
  "/v1/auth/ws-ticket": "/v1/auth/ws-ticket",
  "/v1/auth/sessions": "/v1/auth/sessions",
  "/v1/auth/sessions/revoke-all": "/v1/auth/sessions/revoke-all",
  "/v1/auth/sessions/{session_id}": "/v1/auth/sessions/{session_id}",
  "/v1/auth/forgot-password": "/v1/auth/forgot-password",
  "/v1/auth/reset-password": "/v1/auth/reset-password",
  "/v1/matchmaking/join-casual": "/v1/matchmaking/join-casual",
  "/v1/matchmaking/join-rated-free": "/v1/matchmaking/join-rated-free",
  "/v1/matchmaking/join-rated-staked": "/v1/matchmaking/join-rated-staked",
  "/v1/matchmaking/status/{request_id}": "/v1/matchmaking/status/{request_id}",
  "/v1/matchmaking/match/{match_id}": "/v1/matchmaking/match/{match_id}",
  "/v1/matchmaking/cancel": "/v1/matchmaking/cancel",
  "/v1/games": "/v1/games",
  "/v1/games/live": "/v1/games/live",
  "/v1/games/import": "/v1/games/import",
  "/v1/games/archive-pgn": "/v1/games/archive-pgn",
  "/v1/games/{game_id}": "/v1/games/{game_id}",
  "/v1/games/{game_id}/analysis": "/v1/games/{game_id}/analysis",
  "/v1/games/{game_id}/pgn": "/v1/games/{game_id}/pgn",
  "/v1/games/{game_id}/move": "/v1/games/{game_id}/move",
  "/v1/games/{game_id}/complete": "/v1/games/{game_id}/complete",
  "/v1/games/{game_id}/join": "/v1/games/{game_id}/join",
  "/v1/ws/game/{game_id}": "/v1/ws/game/{game_id}",
  "/v1/players/online": "/v1/players/online",
  "/v1/players/me/export": "/v1/players/me/export",
  "/v1/players/me/preferences": "/v1/players/me/preferences",
  "/v1/players/me/wallet": "/v1/players/me/wallet",
  "/v1/players/me/wallet/challenge": "/v1/players/me/wallet/challenge",
  "/v1/players/me/wallet/verify": "/v1/players/me/wallet/verify",
  "/v1/players/{username}": "/v1/players/{username}",
  "/v1/players/{username}/games": "/v1/players/{username}/games",
  "/v1/players/{player_id}/elo-history": "/v1/players/{player_id}/elo-history",
  "/v1/tournaments": "/v1/tournaments",
  "/v1/tournaments/{tournament_id}/register": "/v1/tournaments/{tournament_id}/register",
  "/v1/openings/explorer": "/v1/openings/explorer",
  "/v1/leaderboard": "/v1/leaderboard",
  "/v1/notifications": "/v1/notifications",
  "/v1/notifications/read-all": "/v1/notifications/read-all",
  "/v1/notifications/{notification_id}/read": "/v1/notifications/{notification_id}/read",
  "/v1/ws/notifications": "/v1/ws/notifications",
  "/v1/puzzles": "/v1/puzzles",
  "/v1/puzzles/submit": "/v1/puzzles/submit",
  "/v1/repertoires/sync": "/v1/repertoires/sync",
  "/v1/season": "/v1/season",
  "/v1/season/standings": "/v1/season/standings",
  "/v1/season/claimable": "/v1/season/claimable",
  "/v1/season/claim": "/v1/season/claim",
} as const;

export type ApiPath = keyof typeof apiPaths;

export interface ApiOperationMap {
  "POST /v1/auth/login": {
    input: ApiCallOptions & { body: LoginRequest };
    response: AuthResponse;
  };
  "POST /v1/auth/register": {
    input: ApiCallOptions & { body: RegisterRequest };
    response: AuthResponse;
  };
  "POST /v1/auth/logout": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/auth/refresh": {
    input: ApiCallOptions;
    response: RefreshResponse;
  };
  "POST /v1/auth/ws-ticket": {
    input: ApiCallOptions;
    response: WsTicketResponse;
  };
  "GET /v1/auth/sessions": {
    input: ApiCallOptions;
    response: Array<SessionInfo>;
  };
  "POST /v1/auth/sessions/revoke-all": {
    input: ApiCallOptions;
    response: void;
  };
  "DELETE /v1/auth/sessions/{session_id}": {
    input: ApiCallOptions & { session_id: string };
    response: void;
  };
  "POST /v1/auth/forgot-password": {
    input: ApiCallOptions & { body: ForgotPasswordRequest };
    response: void;
  };
  "POST /v1/auth/reset-password": {
    input: ApiCallOptions & { body: ResetPasswordRequest };
    response: void;
  };
  "POST /v1/matchmaking/join-casual": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/matchmaking/join-rated-free": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/matchmaking/join-rated-staked": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/matchmaking/status/{request_id}": {
    input: ApiCallOptions & { request_id: string };
    response: void;
  };
  "GET /v1/matchmaking/match/{match_id}": {
    input: ApiCallOptions & { match_id: string };
    response: void;
  };
  "POST /v1/matchmaking/cancel": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/games": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/games": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/games/live": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/games/import": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/games/archive-pgn": {
    input: ApiCallOptions & { body: PgnArchiveRequest };
    response: PgnArchiveResponse;
  };
  "GET /v1/games/{game_id}": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "DELETE /v1/games/{game_id}": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "GET /v1/games/{game_id}/analysis": {
    input: ApiCallOptions & { game_id: string };
    response: GameAnalysisResponse;
  };
  "GET /v1/games/{game_id}/pgn": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "PUT /v1/games/{game_id}/move": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "PUT /v1/games/{game_id}/complete": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "POST /v1/games/{game_id}/join": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "GET /v1/ws/game/{game_id}": {
    input: ApiCallOptions & { game_id: string };
    response: void;
  };
  "GET /v1/players/online": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/players/me/export": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/players/me/preferences": {
    input: ApiCallOptions;
    response: void;
  };
  "PUT /v1/players/me/preferences": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/players/me/wallet": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/players/me/wallet/challenge": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/players/me/wallet/verify": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/players/{username}": {
    input: ApiCallOptions & { username: string };
    response: void;
  };
  "GET /v1/players/{username}/games": {
    input: ApiCallOptions & { username: string };
    response: void;
  };
  "GET /v1/players/{player_id}/elo-history": {
    input: ApiCallOptions & { player_id: string };
    response: void;
  };
  "GET /v1/tournaments": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/tournaments": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/tournaments/{tournament_id}/register": {
    input: ApiCallOptions & { tournament_id: string };
    response: void;
  };
  "GET /v1/openings/explorer": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/leaderboard": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/notifications": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/notifications/read-all": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/notifications/{notification_id}/read": {
    input: ApiCallOptions & { notification_id: string };
    response: void;
  };
  "GET /v1/ws/notifications": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/puzzles": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/puzzles/submit": {
    input: ApiCallOptions;
    response: void;
  };
  "POST /v1/repertoires/sync": {
    input: ApiCallOptions;
    response: void;
  };
  "GET /v1/season": {
    input: ApiCallOptions;
    response: SeasonStatus;
  };
  "GET /v1/season/standings": {
    input: ApiCallOptions;
    response: SeasonStandings;
  };
  "GET /v1/season/claimable": {
    input: ApiCallOptions;
    response: SeasonClaimable;
  };
  "POST /v1/season/claim": {
    input: ApiCallOptions & { body: SeasonClaimRequest };
    response: SeasonClaimResponse;
  };
}

export type LoginInput = ApiCallOptions & { body: LoginRequest };
export type LoginResponse = AuthResponse;

export type RegisterInput = ApiCallOptions & { body: RegisterRequest };
export type RegisterResponse = AuthResponse;

export type LogoutInput = ApiCallOptions;
export type LogoutResponse = void;

export type RefreshInput = ApiCallOptions;
export type RefreshOperationResponse = RefreshResponse;

export type RequestWsTicketInput = ApiCallOptions;
export type RequestWsTicketResponse = WsTicketResponse;

export type ListSessionsInput = ApiCallOptions;
export type ListSessionsResponse = Array<SessionInfo>;

export type RevokeAllSessionsInput = ApiCallOptions;
export type RevokeAllSessionsResponse = void;

export type RevokeSessionInput = ApiCallOptions & { session_id: string };
export type RevokeSessionResponse = void;

export type ForgotPasswordInput = ApiCallOptions & { body: ForgotPasswordRequest };
export type ForgotPasswordResponse = void;

export type ResetPasswordInput = ApiCallOptions & { body: ResetPasswordRequest };
export type ResetPasswordResponse = void;

export type JoinCasualInput = ApiCallOptions;
export type JoinCasualResponse = void;

export type JoinRatedInput = ApiCallOptions;
export type JoinRatedResponse = void;

export type JoinStakedInput = ApiCallOptions;
export type JoinStakedResponse = void;

export type MatchmakingStatusInput = ApiCallOptions & { request_id: string };
export type MatchmakingStatusResponse = void;

export type MatchmakingMatchInput = ApiCallOptions & { match_id: string };
export type MatchmakingMatchResponse = void;

export type CancelMatchmakingInput = ApiCallOptions;
export type CancelMatchmakingResponse = void;

export type ListGamesInput = ApiCallOptions;
export type ListGamesResponse = void;

export type CreateGameInput = ApiCallOptions;
export type CreateGameResponse = void;

export type ListLiveGamesInput = ApiCallOptions;
export type ListLiveGamesResponse = void;

export type ImportGameInput = ApiCallOptions;
export type ImportGameResponse = void;

export type ArchivePgnInput = ApiCallOptions & { body: PgnArchiveRequest };
export type ArchivePgnResponse = PgnArchiveResponse;

export type GetGameInput = ApiCallOptions & { game_id: string };
export type GetGameResponse = void;

export type DeleteGameInput = ApiCallOptions & { game_id: string };
export type DeleteGameResponse = void;

export type GetGameAnalysisInput = ApiCallOptions & { game_id: string };
export type GetGameAnalysisResponse = GameAnalysisResponse;

export type GetGamePgnInput = ApiCallOptions & { game_id: string };
export type GetGamePgnResponse = void;

export type MakeMoveInput = ApiCallOptions & { game_id: string };
export type MakeMoveResponse = void;

export type CompleteGameInput = ApiCallOptions & { game_id: string };
export type CompleteGameResponse = void;

export type JoinGameInput = ApiCallOptions & { game_id: string };
export type JoinGameResponse = void;

export type GameSocketInput = ApiCallOptions & { game_id: string };
export type GameSocketResponse = void;

export type PlayersOnlineInput = ApiCallOptions;
export type PlayersOnlineResponse = void;

export type ExportPlayerInput = ApiCallOptions;
export type ExportPlayerResponse = void;

export type GetPlayerPreferencesInput = ApiCallOptions;
export type GetPlayerPreferencesResponse = void;

export type UpdatePlayerPreferencesInput = ApiCallOptions;
export type UpdatePlayerPreferencesResponse = void;

export type GetLinkedWalletInput = ApiCallOptions;
export type GetLinkedWalletResponse = void;

export type WalletChallengeInput = ApiCallOptions;
export type WalletChallengeResponse = void;

export type WalletVerifyInput = ApiCallOptions;
export type WalletVerifyResponse = void;

export type GetPlayerInput = ApiCallOptions & { username: string };
export type GetPlayerResponse = void;

export type GetPlayerGamesInput = ApiCallOptions & { username: string };
export type GetPlayerGamesResponse = void;

export type GetEloHistoryInput = ApiCallOptions & { player_id: string };
export type GetEloHistoryResponse = void;

export type ListTournamentsInput = ApiCallOptions;
export type ListTournamentsResponse = void;

export type CreateTournamentInput = ApiCallOptions;
export type CreateTournamentResponse = void;

export type RegisterForTournamentInput = ApiCallOptions & { tournament_id: string };
export type RegisterForTournamentResponse = void;

export type ExploreOpeningsInput = ApiCallOptions;
export type ExploreOpeningsResponse = void;

export type GetLeaderboardInput = ApiCallOptions;
export type GetLeaderboardResponse = void;

export type ListNotificationsInput = ApiCallOptions;
export type ListNotificationsResponse = void;

export type MarkAllNotificationsReadInput = ApiCallOptions;
export type MarkAllNotificationsReadResponse = void;

export type MarkNotificationReadInput = ApiCallOptions & { notification_id: string };
export type MarkNotificationReadResponse = void;

export type NotificationsSocketInput = ApiCallOptions;
export type NotificationsSocketResponse = void;

export type ListPuzzlesInput = ApiCallOptions;
export type ListPuzzlesResponse = void;

export type SubmitPuzzleInput = ApiCallOptions;
export type SubmitPuzzleResponse = void;

export type SyncRepertoireInput = ApiCallOptions;
export type SyncRepertoireResponse = void;

export type GetSeasonInput = ApiCallOptions;
export type GetSeasonResponse = SeasonStatus;

export type GetSeasonStandingsInput = ApiCallOptions;
export type GetSeasonStandingsResponse = SeasonStandings;

export type GetSeasonClaimableInput = ApiCallOptions;
export type GetSeasonClaimableResponse = SeasonClaimable;

export type ClaimSeasonPrizeInput = ApiCallOptions & { body: SeasonClaimRequest };
export type ClaimSeasonPrizeResponse = SeasonClaimResponse;

export type ApiOperationKey = keyof ApiOperationMap;

export interface ApiClient {
  login(input: LoginInput): Promise<LoginResponse>;
  register(input: RegisterInput): Promise<RegisterResponse>;
  logout(input?: LogoutInput): Promise<LogoutResponse>;
  refresh(input?: RefreshInput): Promise<RefreshOperationResponse>;
  requestWsTicket(input?: RequestWsTicketInput): Promise<RequestWsTicketResponse>;
  listSessions(input?: ListSessionsInput): Promise<ListSessionsResponse>;
  revokeAllSessions(input?: RevokeAllSessionsInput): Promise<RevokeAllSessionsResponse>;
  revokeSession(input: RevokeSessionInput): Promise<RevokeSessionResponse>;
  forgotPassword(input: ForgotPasswordInput): Promise<ForgotPasswordResponse>;
  resetPassword(input: ResetPasswordInput): Promise<ResetPasswordResponse>;
  joinCasual(input?: JoinCasualInput): Promise<JoinCasualResponse>;
  joinRated(input?: JoinRatedInput): Promise<JoinRatedResponse>;
  joinStaked(input?: JoinStakedInput): Promise<JoinStakedResponse>;
  matchmakingStatus(input: MatchmakingStatusInput): Promise<MatchmakingStatusResponse>;
  matchmakingMatch(input: MatchmakingMatchInput): Promise<MatchmakingMatchResponse>;
  cancelMatchmaking(input?: CancelMatchmakingInput): Promise<CancelMatchmakingResponse>;
  listGames(input?: ListGamesInput): Promise<ListGamesResponse>;
  createGame(input?: CreateGameInput): Promise<CreateGameResponse>;
  listLiveGames(input?: ListLiveGamesInput): Promise<ListLiveGamesResponse>;
  importGame(input?: ImportGameInput): Promise<ImportGameResponse>;
  archivePgn(input: ArchivePgnInput): Promise<ArchivePgnResponse>;
  getGame(input: GetGameInput): Promise<GetGameResponse>;
  deleteGame(input: DeleteGameInput): Promise<DeleteGameResponse>;
  getGameAnalysis(input: GetGameAnalysisInput): Promise<GetGameAnalysisResponse>;
  getGamePgn(input: GetGamePgnInput): Promise<GetGamePgnResponse>;
  makeMove(input: MakeMoveInput): Promise<MakeMoveResponse>;
  completeGame(input: CompleteGameInput): Promise<CompleteGameResponse>;
  joinGame(input: JoinGameInput): Promise<JoinGameResponse>;
  gameSocket(input: GameSocketInput): Promise<GameSocketResponse>;
  playersOnline(input?: PlayersOnlineInput): Promise<PlayersOnlineResponse>;
  exportPlayer(input?: ExportPlayerInput): Promise<ExportPlayerResponse>;
  getPlayerPreferences(input?: GetPlayerPreferencesInput): Promise<GetPlayerPreferencesResponse>;
  updatePlayerPreferences(input?: UpdatePlayerPreferencesInput): Promise<UpdatePlayerPreferencesResponse>;
  getLinkedWallet(input?: GetLinkedWalletInput): Promise<GetLinkedWalletResponse>;
  walletChallenge(input?: WalletChallengeInput): Promise<WalletChallengeResponse>;
  walletVerify(input?: WalletVerifyInput): Promise<WalletVerifyResponse>;
  getPlayer(input: GetPlayerInput): Promise<GetPlayerResponse>;
  getPlayerGames(input: GetPlayerGamesInput): Promise<GetPlayerGamesResponse>;
  getEloHistory(input: GetEloHistoryInput): Promise<GetEloHistoryResponse>;
  listTournaments(input?: ListTournamentsInput): Promise<ListTournamentsResponse>;
  createTournament(input?: CreateTournamentInput): Promise<CreateTournamentResponse>;
  registerForTournament(input: RegisterForTournamentInput): Promise<RegisterForTournamentResponse>;
  exploreOpenings(input?: ExploreOpeningsInput): Promise<ExploreOpeningsResponse>;
  getLeaderboard(input?: GetLeaderboardInput): Promise<GetLeaderboardResponse>;
  listNotifications(input?: ListNotificationsInput): Promise<ListNotificationsResponse>;
  markAllNotificationsRead(input?: MarkAllNotificationsReadInput): Promise<MarkAllNotificationsReadResponse>;
  markNotificationRead(input: MarkNotificationReadInput): Promise<MarkNotificationReadResponse>;
  notificationsSocket(input?: NotificationsSocketInput): Promise<NotificationsSocketResponse>;
  listPuzzles(input?: ListPuzzlesInput): Promise<ListPuzzlesResponse>;
  submitPuzzle(input?: SubmitPuzzleInput): Promise<SubmitPuzzleResponse>;
  syncRepertoire(input?: SyncRepertoireInput): Promise<SyncRepertoireResponse>;
  getSeason(input?: GetSeasonInput): Promise<GetSeasonResponse>;
  getSeasonStandings(input?: GetSeasonStandingsInput): Promise<GetSeasonStandingsResponse>;
  getSeasonClaimable(input?: GetSeasonClaimableInput): Promise<GetSeasonClaimableResponse>;
  claimSeasonPrize(input: ClaimSeasonPrizeInput): Promise<ClaimSeasonPrizeResponse>;
}

async function request<K extends ApiOperationKey>(
  baseUrl: string,
  path: string,
  method: string,
  input: ApiOperationMap[K]["input"],
): Promise<ApiOperationMap[K]["response"]> {
  const options = input as ApiCallOptions & { body?: unknown };
  const url = path.replace(/\{([^}]+)\}/g, (_, key: string) =>
    encodeURIComponent(String((options as Record<string, unknown>)[key])),
  );
  const headers = new Headers(options.headers);
  if (options.body !== undefined && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }
  if (options.token && !headers.has("Authorization")) {
    headers.set("Authorization", `Bearer ${options.token}`);
  }
  const response = await fetchWithRetry(
    `${baseUrl}${url}`,
    {
      method,
      headers,
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      credentials: options.credentials,
      signal: options.signal,
    },
    options.retryOptions,
  );
  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try {
      const body: unknown = await response.json();
      if (body && typeof body === "object" && typeof (body as { message?: unknown }).message === "string") {
        message = (body as { message: string }).message;
      }
    } catch {
    }
    throw new ApiError(response.status, message);
  }
  if (response.status === 204) return undefined as ApiOperationMap[K]["response"];
  try {
    return (await response.json()) as ApiOperationMap[K]["response"];
  } catch {
    return undefined as ApiOperationMap[K]["response"];
  }
}

export interface ApiClientOptions {
  baseUrl?: string;
}

export function createApiClient(options: ApiClientOptions = {}): ApiClient {
  const baseUrl = (options.baseUrl ?? API_BASE).replace(/\/$/, "");
  return {
    login: (input: ApiOperationMap["POST /v1/auth/login"]["input"]) =>
      request<"POST /v1/auth/login">(baseUrl, "/v1/auth/login", "POST", input),
    register: (input: ApiOperationMap["POST /v1/auth/register"]["input"]) =>
      request<"POST /v1/auth/register">(baseUrl, "/v1/auth/register", "POST", input),
    logout: (input: ApiOperationMap["POST /v1/auth/logout"]["input"] = {}) =>
      request<"POST /v1/auth/logout">(baseUrl, "/v1/auth/logout", "POST", input),
    refresh: (input: ApiOperationMap["POST /v1/auth/refresh"]["input"] = {}) =>
      request<"POST /v1/auth/refresh">(baseUrl, "/v1/auth/refresh", "POST", input),
    requestWsTicket: (input: ApiOperationMap["POST /v1/auth/ws-ticket"]["input"] = {}) =>
      request<"POST /v1/auth/ws-ticket">(baseUrl, "/v1/auth/ws-ticket", "POST", input),
    listSessions: (input: ApiOperationMap["GET /v1/auth/sessions"]["input"] = {}) =>
      request<"GET /v1/auth/sessions">(baseUrl, "/v1/auth/sessions", "GET", input),
    revokeAllSessions: (input: ApiOperationMap["POST /v1/auth/sessions/revoke-all"]["input"] = {}) =>
      request<"POST /v1/auth/sessions/revoke-all">(baseUrl, "/v1/auth/sessions/revoke-all", "POST", input),
    revokeSession: (input: ApiOperationMap["DELETE /v1/auth/sessions/{session_id}"]["input"]) =>
      request<"DELETE /v1/auth/sessions/{session_id}">(baseUrl, "/v1/auth/sessions/{session_id}", "DELETE", input),
    forgotPassword: (input: ApiOperationMap["POST /v1/auth/forgot-password"]["input"]) =>
      request<"POST /v1/auth/forgot-password">(baseUrl, "/v1/auth/forgot-password", "POST", input),
    resetPassword: (input: ApiOperationMap["POST /v1/auth/reset-password"]["input"]) =>
      request<"POST /v1/auth/reset-password">(baseUrl, "/v1/auth/reset-password", "POST", input),
    joinCasual: (input: ApiOperationMap["POST /v1/matchmaking/join-casual"]["input"] = {}) =>
      request<"POST /v1/matchmaking/join-casual">(baseUrl, "/v1/matchmaking/join-casual", "POST", input),
    joinRated: (input: ApiOperationMap["POST /v1/matchmaking/join-rated-free"]["input"] = {}) =>
      request<"POST /v1/matchmaking/join-rated-free">(baseUrl, "/v1/matchmaking/join-rated-free", "POST", input),
    joinStaked: (input: ApiOperationMap["POST /v1/matchmaking/join-rated-staked"]["input"] = {}) =>
      request<"POST /v1/matchmaking/join-rated-staked">(baseUrl, "/v1/matchmaking/join-rated-staked", "POST", input),
    matchmakingStatus: (input: ApiOperationMap["GET /v1/matchmaking/status/{request_id}"]["input"]) =>
      request<"GET /v1/matchmaking/status/{request_id}">(baseUrl, "/v1/matchmaking/status/{request_id}", "GET", input),
    matchmakingMatch: (input: ApiOperationMap["GET /v1/matchmaking/match/{match_id}"]["input"]) =>
      request<"GET /v1/matchmaking/match/{match_id}">(baseUrl, "/v1/matchmaking/match/{match_id}", "GET", input),
    cancelMatchmaking: (input: ApiOperationMap["POST /v1/matchmaking/cancel"]["input"] = {}) =>
      request<"POST /v1/matchmaking/cancel">(baseUrl, "/v1/matchmaking/cancel", "POST", input),
    listGames: (input: ApiOperationMap["GET /v1/games"]["input"] = {}) =>
      request<"GET /v1/games">(baseUrl, "/v1/games", "GET", input),
    createGame: (input: ApiOperationMap["POST /v1/games"]["input"] = {}) =>
      request<"POST /v1/games">(baseUrl, "/v1/games", "POST", input),
    listLiveGames: (input: ApiOperationMap["GET /v1/games/live"]["input"] = {}) =>
      request<"GET /v1/games/live">(baseUrl, "/v1/games/live", "GET", input),
    importGame: (input: ApiOperationMap["POST /v1/games/import"]["input"] = {}) =>
      request<"POST /v1/games/import">(baseUrl, "/v1/games/import", "POST", input),
    archivePgn: (input: ApiOperationMap["POST /v1/games/archive-pgn"]["input"]) =>
      request<"POST /v1/games/archive-pgn">(baseUrl, "/v1/games/archive-pgn", "POST", input),
    getGame: (input: ApiOperationMap["GET /v1/games/{game_id}"]["input"]) =>
      request<"GET /v1/games/{game_id}">(baseUrl, "/v1/games/{game_id}", "GET", input),
    deleteGame: (input: ApiOperationMap["DELETE /v1/games/{game_id}"]["input"]) =>
      request<"DELETE /v1/games/{game_id}">(baseUrl, "/v1/games/{game_id}", "DELETE", input),
    getGameAnalysis: (input: ApiOperationMap["GET /v1/games/{game_id}/analysis"]["input"]) =>
      request<"GET /v1/games/{game_id}/analysis">(baseUrl, "/v1/games/{game_id}/analysis", "GET", input),
    getGamePgn: (input: ApiOperationMap["GET /v1/games/{game_id}/pgn"]["input"]) =>
      request<"GET /v1/games/{game_id}/pgn">(baseUrl, "/v1/games/{game_id}/pgn", "GET", input),
    makeMove: (input: ApiOperationMap["PUT /v1/games/{game_id}/move"]["input"]) =>
      request<"PUT /v1/games/{game_id}/move">(baseUrl, "/v1/games/{game_id}/move", "PUT", input),
    completeGame: (input: ApiOperationMap["PUT /v1/games/{game_id}/complete"]["input"]) =>
      request<"PUT /v1/games/{game_id}/complete">(baseUrl, "/v1/games/{game_id}/complete", "PUT", input),
    joinGame: (input: ApiOperationMap["POST /v1/games/{game_id}/join"]["input"]) =>
      request<"POST /v1/games/{game_id}/join">(baseUrl, "/v1/games/{game_id}/join", "POST", input),
    gameSocket: (input: ApiOperationMap["GET /v1/ws/game/{game_id}"]["input"]) =>
      request<"GET /v1/ws/game/{game_id}">(baseUrl, "/v1/ws/game/{game_id}", "GET", input),
    playersOnline: (input: ApiOperationMap["GET /v1/players/online"]["input"] = {}) =>
      request<"GET /v1/players/online">(baseUrl, "/v1/players/online", "GET", input),
    exportPlayer: (input: ApiOperationMap["GET /v1/players/me/export"]["input"] = {}) =>
      request<"GET /v1/players/me/export">(baseUrl, "/v1/players/me/export", "GET", input),
    getPlayerPreferences: (input: ApiOperationMap["GET /v1/players/me/preferences"]["input"] = {}) =>
      request<"GET /v1/players/me/preferences">(baseUrl, "/v1/players/me/preferences", "GET", input),
    updatePlayerPreferences: (input: ApiOperationMap["PUT /v1/players/me/preferences"]["input"] = {}) =>
      request<"PUT /v1/players/me/preferences">(baseUrl, "/v1/players/me/preferences", "PUT", input),
    getLinkedWallet: (input: ApiOperationMap["GET /v1/players/me/wallet"]["input"] = {}) =>
      request<"GET /v1/players/me/wallet">(baseUrl, "/v1/players/me/wallet", "GET", input),
    walletChallenge: (input: ApiOperationMap["POST /v1/players/me/wallet/challenge"]["input"] = {}) =>
      request<"POST /v1/players/me/wallet/challenge">(baseUrl, "/v1/players/me/wallet/challenge", "POST", input),
    walletVerify: (input: ApiOperationMap["POST /v1/players/me/wallet/verify"]["input"] = {}) =>
      request<"POST /v1/players/me/wallet/verify">(baseUrl, "/v1/players/me/wallet/verify", "POST", input),
    getPlayer: (input: ApiOperationMap["GET /v1/players/{username}"]["input"]) =>
      request<"GET /v1/players/{username}">(baseUrl, "/v1/players/{username}", "GET", input),
    getPlayerGames: (input: ApiOperationMap["GET /v1/players/{username}/games"]["input"]) =>
      request<"GET /v1/players/{username}/games">(baseUrl, "/v1/players/{username}/games", "GET", input),
    getEloHistory: (input: ApiOperationMap["GET /v1/players/{player_id}/elo-history"]["input"]) =>
      request<"GET /v1/players/{player_id}/elo-history">(baseUrl, "/v1/players/{player_id}/elo-history", "GET", input),
    listTournaments: (input: ApiOperationMap["GET /v1/tournaments"]["input"] = {}) =>
      request<"GET /v1/tournaments">(baseUrl, "/v1/tournaments", "GET", input),
    createTournament: (input: ApiOperationMap["POST /v1/tournaments"]["input"] = {}) =>
      request<"POST /v1/tournaments">(baseUrl, "/v1/tournaments", "POST", input),
    registerForTournament: (input: ApiOperationMap["POST /v1/tournaments/{tournament_id}/register"]["input"]) =>
      request<"POST /v1/tournaments/{tournament_id}/register">(baseUrl, "/v1/tournaments/{tournament_id}/register", "POST", input),
    exploreOpenings: (input: ApiOperationMap["GET /v1/openings/explorer"]["input"] = {}) =>
      request<"GET /v1/openings/explorer">(baseUrl, "/v1/openings/explorer", "GET", input),
    getLeaderboard: (input: ApiOperationMap["GET /v1/leaderboard"]["input"] = {}) =>
      request<"GET /v1/leaderboard">(baseUrl, "/v1/leaderboard", "GET", input),
    listNotifications: (input: ApiOperationMap["GET /v1/notifications"]["input"] = {}) =>
      request<"GET /v1/notifications">(baseUrl, "/v1/notifications", "GET", input),
    markAllNotificationsRead: (input: ApiOperationMap["POST /v1/notifications/read-all"]["input"] = {}) =>
      request<"POST /v1/notifications/read-all">(baseUrl, "/v1/notifications/read-all", "POST", input),
    markNotificationRead: (input: ApiOperationMap["POST /v1/notifications/{notification_id}/read"]["input"]) =>
      request<"POST /v1/notifications/{notification_id}/read">(baseUrl, "/v1/notifications/{notification_id}/read", "POST", input),
    notificationsSocket: (input: ApiOperationMap["GET /v1/ws/notifications"]["input"] = {}) =>
      request<"GET /v1/ws/notifications">(baseUrl, "/v1/ws/notifications", "GET", input),
    listPuzzles: (input: ApiOperationMap["GET /v1/puzzles"]["input"] = {}) =>
      request<"GET /v1/puzzles">(baseUrl, "/v1/puzzles", "GET", input),
    submitPuzzle: (input: ApiOperationMap["POST /v1/puzzles/submit"]["input"] = {}) =>
      request<"POST /v1/puzzles/submit">(baseUrl, "/v1/puzzles/submit", "POST", input),
    syncRepertoire: (input: ApiOperationMap["POST /v1/repertoires/sync"]["input"] = {}) =>
      request<"POST /v1/repertoires/sync">(baseUrl, "/v1/repertoires/sync", "POST", input),
    getSeason: (input: ApiOperationMap["GET /v1/season"]["input"] = {}) =>
      request<"GET /v1/season">(baseUrl, "/v1/season", "GET", input),
    getSeasonStandings: (input: ApiOperationMap["GET /v1/season/standings"]["input"] = {}) =>
      request<"GET /v1/season/standings">(baseUrl, "/v1/season/standings", "GET", input),
    getSeasonClaimable: (input: ApiOperationMap["GET /v1/season/claimable"]["input"] = {}) =>
      request<"GET /v1/season/claimable">(baseUrl, "/v1/season/claimable", "GET", input),
    claimSeasonPrize: (input: ApiOperationMap["POST /v1/season/claim"]["input"]) =>
      request<"POST /v1/season/claim">(baseUrl, "/v1/season/claim", "POST", input),
  };
}

export const apiClient = createApiClient();
