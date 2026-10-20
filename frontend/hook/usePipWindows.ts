"use client";

import { useCallback, useEffect, useState } from "react";

import {
  bringToFront,
  clampRectToViewport,
  closePipWindow,
  openPipWindow,
  updatePipRect,
  type PipRect,
  type PipWindowState,
  type ViewportSize,
} from "@/lib/pipWindow";

export interface UsePipWindowsResult {
  windows: PipWindowState[];
  viewport: ViewportSize;
  isOpen: (gameId: string) => boolean;
  open: (gameId: string) => void;
  close: (gameId: string) => void;
  focus: (gameId: string) => void;
  setRect: (gameId: string, rect: PipRect) => void;
  closeAll: () => void;
}

const DEFAULT_VIEWPORT: ViewportSize = { width: 1280, height: 800 };

/**
 * Owns the set of floating PIP windows for the spectator lobby:
 * open/close/focus plus viewport tracking. Geometry lives in
 * `lib/pipWindow.ts`; this hook only keeps React state in sync with it.
 */
export function usePipWindows(initialViewport: ViewportSize = DEFAULT_VIEWPORT): UsePipWindowsResult {
  const [windows, setWindows] = useState<PipWindowState[]>([]);
  const [viewport, setViewport] = useState<ViewportSize>(initialViewport);

  useEffect(() => {
    const measure = () =>
      setViewport({ width: window.innerWidth, height: window.innerHeight });

    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, []);

  // Keep pinned windows on-screen when the viewport shrinks (rotation, split
  // view, dev-tools opening).
  useEffect(() => {
    setWindows((current) =>
      current.map((window) => ({ ...window, rect: clampRectToViewport(window.rect, viewport) })),
    );
  }, [viewport.width, viewport.height]);

  const isOpen = useCallback(
    (gameId: string) => windows.some((window) => window.gameId === gameId),
    [windows],
  );

  const open = useCallback(
    (gameId: string) => {
      setWindows((current) => openPipWindow(current, gameId, viewport));
    },
    [viewport],
  );

  const close = useCallback((gameId: string) => {
    setWindows((current) => closePipWindow(current, gameId));
  }, []);

  const focus = useCallback((gameId: string) => {
    setWindows((current) => bringToFront(current, gameId));
  }, []);

  const setRect = useCallback((gameId: string, rect: PipRect) => {
    setWindows((current) => updatePipRect(current, gameId, rect));
  }, []);

  const closeAll = useCallback(() => setWindows([]), []);

  return { windows, viewport, isOpen, open, close, focus, setRect, closeAll };
}
