export interface SpectatorMessage {
  user: string;
  message: string;
  emoji?: string;
  timestamp: Date;
}

export const SpectatorChat = () => {
  const sendMessage = (msg: string, emoji?: string) => {
    const message: SpectatorMessage = {
      user: 'spectator', message: msg, emoji, timestamp: new Date()
    };
  };
  return null;
};
