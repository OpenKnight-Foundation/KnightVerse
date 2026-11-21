"use client";

import { forwardRef, useEffect, useState } from "react";

interface GameChatPanelProps {
  open: boolean;
  onClose: () => void;
}

interface ChatMessage {
  id: number;
  text: string;
}

export const GameChatPanel = forwardRef<HTMLInputElement, GameChatPanelProps>(
  function GameChatPanel({ open, onClose }, ref) {
    const [draft, setDraft] = useState("");
    const [messages, setMessages] = useState<ChatMessage[]>([]);

    useEffect(() => {
      if (!open) return;
      const handleKeyDown = (event: KeyboardEvent) => {
        if (event.key === "Escape") {
          event.preventDefault();
          onClose();
        }
      };
      window.addEventListener("keydown", handleKeyDown);
      return () => window.removeEventListener("keydown", handleKeyDown);
    }, [open, onClose]);

    if (!open) return null;

    const send = (event: React.FormEvent) => {
      event.preventDefault();
      const text = draft.trim();
      if (!text) return;
      setMessages((prev) => [...prev, { id: Date.now(), text }]);
      setDraft("");
    };

    return (
      <div
        className="fixed bottom-4 right-4 z-[110] w-[min(360px,calc(100vw-2rem))] rounded-xl border border-teal-500/40 bg-gray-900 p-4 text-white shadow-2xl"
        role="dialog"
        aria-modal="false"
        aria-label="Game chat"
      >
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-sm font-bold text-teal-300">Game Chat</h2>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close game chat"
            className="rounded px-2 py-1 text-gray-400 hover:bg-gray-800 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
          >
            ✕
          </button>
        </div>
        <div className="max-h-48 space-y-1 overflow-y-auto" aria-live="polite">
          {messages.length === 0 ? (
            <p className="text-xs text-gray-500">No messages yet. Say hello!</p>
          ) : (
            messages.map((message) => (
              <p key={message.id} className="rounded bg-gray-800/70 px-2 py-1 text-xs">
                <span className="font-semibold text-teal-300">You: </span>
                {message.text}
              </p>
            ))
          )}
        </div>
        <form onSubmit={send} className="mt-3 flex gap-2">
          <label htmlFor="game-chat-input" className="sr-only">
            Chat message
          </label>
          <input
            ref={ref}
            id="game-chat-input"
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="Type a message…"
            autoComplete="off"
            className="min-w-0 flex-1 rounded-lg border border-gray-700 bg-gray-800 px-3 py-2 text-sm outline-none focus:border-teal-500 focus:ring-2 focus:ring-teal-500/40"
          />
          <button
            type="submit"
            disabled={!draft.trim()}
            className="rounded-lg bg-teal-600 px-3 py-2 text-sm font-semibold disabled:opacity-50"
          >
            Send
          </button>
        </form>
      </div>
    );
  },
);

export default GameChatPanel;
