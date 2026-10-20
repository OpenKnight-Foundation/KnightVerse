// A single chat message sent by a spectator: who sent it, the text, an
// optional emoji reaction, and when it was sent.
export interface SpectatorMessage {
  user: string;
  message: string;
  emoji?: string;
  timestamp: Date;
}

// Placeholder component for a live spectator chat — currently
// unimplemented: it always renders nothing (`return null`), and its UI
// (message list, input, etc.) still needs to be built.
export const SpectatorChat = () => {
  // Builds a SpectatorMessage from the given text/emoji, hardcoding the
  // sender as "spectator" rather than using a real logged-in user.
  //
  // Note: the constructed `message` is never stored, sent, or returned —
  // this function currently has no observable effect. It'll need to
  // either be appended to local/shared state or sent to a backend/socket
  // once the chat is actually implemented.
  const sendMessage = (msg: string, emoji?: string) => {
    const message: SpectatorMessage = {
      user: 'spectator', message: msg, emoji, timestamp: new Date()
    };
  };
  return null;
};