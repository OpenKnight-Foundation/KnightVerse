"use client";

import { useSpectatorSocket } from "@/hook/useSpectatorSocket";
import type { PipRect, PipWindowState, ViewportSize } from "@/lib/pipWindow";

import { PictureInPictureBoard } from "./PictureInPictureBoard";

export interface PipWindowLayerProps {
  windows: PipWindowState[];
  viewport: ViewportSize;
  /** Game currently open in full spectator mode, if any. */
  activeGameId?: string | null;
  onFocus: (gameId: string) => void;
  onClose: (gameId: string) => void;
  onExpand: (gameId: string) => void;
  onRectChange: (gameId: string, rect: PipRect) => void;
}

interface LivePipWindowProps extends Omit<PipWindowLayerProps, "windows" | "activeGameId"> {
  window: PipWindowState;
  isActive: boolean;
}

/**
 * Subscribes one PIP window to its game's spectator feed. Kept as a discrete
 * component so each window owns its own socket lifecycle (hooks may not be
 * called in a loop) and unmounting a window releases only that feed.
 */
function LivePipWindow({
  window,
  viewport,
  isActive,
  onFocus,
  onClose,
  onExpand,
  onRectChange,
}: LivePipWindowProps) {
  const { status, gameState } = useSpectatorSocket(window.gameId);

  return (
    <PictureInPictureBoard
      gameId={window.gameId}
      rect={window.rect}
      zIndex={window.zIndex}
      viewport={viewport}
      isActive={isActive}
      white={gameState?.white}
      black={gameState?.black}
      fen={gameState?.fen}
      moves={gameState?.moves}
      status={status}
      onFocus={onFocus}
      onClose={onClose}
      onExpand={onExpand}
      onRectChange={onRectChange}
    />
  );
}

/**
 * Renders every pinned game as a floating window above the spectator lobby.
 * The container ignores pointer events so the lobby stays interactive between
 * windows; the windows themselves re-enable them.
 */
export function PipWindowLayer({
  windows,
  viewport,
  activeGameId,
  onFocus,
  onClose,
  onExpand,
  onRectChange,
}: PipWindowLayerProps) {
  if (windows.length === 0) {
    return null;
  }

  return (
    <div
      data-testid="pip-layer"
      className="pointer-events-none fixed inset-0 z-40"
      aria-label="Picture-in-picture games"
    >
      {windows.map((window) => (
        <LivePipWindow
          key={window.id}
          window={window}
          viewport={viewport}
          isActive={activeGameId === window.gameId}
          onFocus={onFocus}
          onClose={onClose}
          onExpand={onExpand}
          onRectChange={onRectChange}
        />
      ))}
    </div>
  );
}
