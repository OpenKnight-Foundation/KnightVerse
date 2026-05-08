/**
 * Matchmaking queue service (#555)
 *
 * Thin client wrapper around the backend Redis-based matchmaking API.
 * Handles joining/leaving the queue, polling for a match, and private invites.
 */

export type MatchType = "Rated" | "Casual" | "Private";

export interface MatchRequest {
  walletAddress: string;
  elo: number;
  matchType: MatchType;
  inviteAddress?: string;
  maxEloDiff?: number;
}

export interface MatchmakingResponse {
  status: string;
  matchId?: string;
  requestId: string;
}

export interface QueueStatus {
  requestId: string;
  position: number;
  estimatedWaitSeconds: number;
  matchType: MatchType;
}

const BASE_URL =
  process.env.NEXT_PUBLIC_BACKEND_URL ?? "http://localhost:8080";

/** Join the matchmaking queue. Returns a requestId to poll with. */
export async function joinQueue(
  request: MatchRequest,
): Promise<MatchmakingResponse> {
  const res = await fetch(`${BASE_URL}/matchmaking/join`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      player: {
        wallet_address: request.walletAddress,
        elo: request.elo,
        join_time: new Date().toISOString(),
      },
      match_type: request.matchType,
      invite_address: request.inviteAddress ?? null,
      max_elo_diff: request.maxEloDiff ?? null,
    }),
  });

  if (!res.ok) {
    throw new Error(`Failed to join queue: ${res.statusText}`);
  }

  return res.json() as Promise<MatchmakingResponse>;
}

/** Poll queue status for a pending request. */
export async function getQueueStatus(
  requestId: string,
): Promise<QueueStatus | null> {
  const res = await fetch(
    `${BASE_URL}/matchmaking/status/${encodeURIComponent(requestId)}`,
  );

  if (res.status === 404) return null;

  if (!res.ok) {
    throw new Error(`Failed to get queue status: ${res.statusText}`);
  }

  return res.json() as Promise<QueueStatus>;
}

/** Leave the matchmaking queue. */
export async function leaveQueue(requestId: string): Promise<void> {
  const res = await fetch(
    `${BASE_URL}/matchmaking/leave/${encodeURIComponent(requestId)}`,
    { method: "DELETE" },
  );

  if (!res.ok && res.status !== 404) {
    throw new Error(`Failed to leave queue: ${res.statusText}`);
  }
}

/** Accept a private match invite. */
export async function acceptPrivateInvite(
  inviterRequestId: string,
  walletAddress: string,
  elo: number,
): Promise<MatchmakingResponse | null> {
  const res = await fetch(`${BASE_URL}/matchmaking/accept-invite`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      inviter_request_id: inviterRequestId,
      accepting_player: {
        wallet_address: walletAddress,
        elo,
        join_time: new Date().toISOString(),
      },
    }),
  });

  if (res.status === 404) return null;

  if (!res.ok) {
    throw new Error(`Failed to accept invite: ${res.statusText}`);
  }

  return res.json() as Promise<MatchmakingResponse>;
}
