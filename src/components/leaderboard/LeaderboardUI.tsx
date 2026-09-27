// Represents a single row in a leaderboard: a player's rank, name,
// rating, and win count.
export interface LeaderboardEntry {
  rank: number;
  player: string;
  rating: number;
  wins: number;
}

// Placeholder components — currently unimplemented stubs that always
// render nothing (`=> null`), despite accepting `entries` as a prop.
// Intended to eventually render a leaderboard table/list from the given
// entries; replace the `null` body with actual markup when ready.
export const GlobalLeaderboard = ({ entries }: { entries: LeaderboardEntry[] }) => null;
export const FriendsLeaderboard = ({ entries }: { entries: LeaderboardEntry[] }) => null;