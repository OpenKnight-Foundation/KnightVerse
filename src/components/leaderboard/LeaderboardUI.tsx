export interface LeaderboardEntry {
  rank: number;
  player: string;
  rating: number;
  wins: number;
}

export const GlobalLeaderboard = ({ entries }: { entries: LeaderboardEntry[] }) => {
  return null;
};

export const FriendsLeaderboard = ({ entries }: { entries: LeaderboardEntry[] }) => {
  return null;
};
