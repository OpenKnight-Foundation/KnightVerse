// Slashing Logic for Disconnected Players (#528)
// Penalizes staked XLM when a player disconnects mid-game without reconnecting

interface StakeRecord {
  playerId: string;
  gameId: string;
  stakedXlm: number;
  connectedAt: number;
}

interface SlashEvent {
  playerId: string;
  gameId: string;
  slashedXlm: number;
  remainingXlm: number;
  reason: string;
  timestamp: number;
}

const SLASH_RATE = 0.25; // 25% of stake slashed on disconnect
const RECONNECT_GRACE_MS = 30_000; // 30 seconds grace period

const disconnectTimestamps = new Map<string, number>();

export function recordDisconnect(playerId: string, gameId: string): void {
  const key = `${gameId}:${playerId}`;
  disconnectTimestamps.set(key, Date.now());
  console.log(`[slash] Player ${playerId} disconnected from game ${gameId}`);
}

export function recordReconnect(playerId: string, gameId: string): boolean {
  const key = `${gameId}:${playerId}`;
  const disconnectedAt = disconnectTimestamps.get(key);
  if (!disconnectedAt) return false;
  const elapsed = Date.now() - disconnectedAt;
  disconnectTimestamps.delete(key);
  return elapsed <= RECONNECT_GRACE_MS;
}

export function evaluateSlash(stake: StakeRecord, reconnectedInTime: boolean): SlashEvent | null {
  if (reconnectedInTime) return null;

  const slashedXlm = parseFloat((stake.stakedXlm * SLASH_RATE).toFixed(7));
  const remainingXlm = parseFloat((stake.stakedXlm - slashedXlm).toFixed(7));

  return {
    playerId: stake.playerId,
    gameId: stake.gameId,
    slashedXlm,
    remainingXlm,
    reason: "Disconnect without reconnect within grace period",
    timestamp: Date.now(),
  };
}

export function processDisconnectSlash(stake: StakeRecord): SlashEvent | null {
  const key = `${stake.gameId}:${stake.playerId}`;
  const disconnectedAt = disconnectTimestamps.get(key);
  if (!disconnectedAt) return null;

  const elapsed = Date.now() - disconnectedAt;
  const reconnectedInTime = elapsed <= RECONNECT_GRACE_MS;
  disconnectTimestamps.delete(key);
  return evaluateSlash(stake, reconnectedInTime);
}
