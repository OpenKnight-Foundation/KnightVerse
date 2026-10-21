"use client";

import React, { createContext, useContext, useState, useEffect, useMemo } from "react";
import {
  fetchPlayerPreferences,
  savePlayerPreferences,
} from "@/services/playerPreferencesService";
import { useAuthOptional } from "@/context/authContext";

export type PieceInputMethod = "drag" | "click" | "both";
export type AutoQueenMode = "always" | "prompt" | "premoves";
export type LegalMoveDots = "enabled" | "disabled";
export type BoardCoordinates = "inside" | "outside" | "hidden";
export type PieceSet = "neo" | "staunton" | "alpha" | "medieval" | "cyberpunk";
export type BoardRenderer = "2d" | "3d";

export interface GamePreferences {
  pieceInputMethod: PieceInputMethod;
  autoQueen: AutoQueenMode;
  showLegalMoveDots: LegalMoveDots;
  confirmMoveCorrespondence: boolean;
  boardCoordinates: BoardCoordinates;
  pieceSet: PieceSet;
  blindfoldMode: boolean;
  voiceMoveEnabled: boolean;
  ttsOpponentMoves: boolean;
  ttsVolume: number;
  ttsRate: number;
  /**
   * Which chessboard renderer to use (FE-123 / #1364). The canonical board is
   * the 2D `ChessboardComponent`; the WebGL 3D board is opt-in behind this
   * explicit appearance setting so only one implementation is live by default.
   */
  boardRenderer: BoardRenderer;
}

export const DEFAULT_PREFERENCES: GamePreferences = {
  pieceInputMethod: "both",
  autoQueen: "always",
  showLegalMoveDots: "enabled",
  confirmMoveCorrespondence: false,
  boardCoordinates: "inside",
  pieceSet: "neo",
  blindfoldMode: false,
  voiceMoveEnabled: true,
  ttsOpponentMoves: true,
  ttsVolume: 80,
  ttsRate: 1,
  boardRenderer: "2d",
};

const STORAGE_KEY = "knightverse_game_preferences";

interface GamePreferencesContextProps {
  preferences: GamePreferences;
  setPreference: <K extends keyof GamePreferences>(
    key: K,
    value: GamePreferences[K],
  ) => void;
  resetPreferences: () => void;
}

const GamePreferencesContext = createContext<
  GamePreferencesContextProps | undefined
>(undefined);

export const GamePreferencesProvider: React.FC<{
  children: React.ReactNode;
}> = ({ children }) => {
  const [preferences, setPreferences] = useState<GamePreferences>(
    DEFAULT_PREFERENCES,
  );
  const [mounted, setMounted] = useState(false);

  const auth = useAuthOptional();
  const authDeps = useMemo(
    () => ({
      getToken: () => auth?.accessToken ?? null,
      refresh: auth?.refreshToken ?? (async () => {}),
      logout: auth?.logout ?? (async () => {}),
    }),
    [auth],
  );

  useEffect(() => {
    let active = true;
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        setPreferences({ ...DEFAULT_PREFERENCES, ...parsed });
      }
    } catch {
      // ignore malformed storage
    }
    setMounted(true);

    void fetchPlayerPreferences(authDeps)
      .then((saved) => {
        const game = saved?.game;
        if (!active || !game || typeof game !== "object" || Array.isArray(game)) {
          return;
        }
        const next = { ...DEFAULT_PREFERENCES, ...game };
        setPreferences(next);
        try {
          localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
        } catch {
          // storage unavailable
        }
      })
      .catch((error) => {
        console.warn("Could not load game preferences from the backend:", error);
      });

    return () => {
      active = false;
    };
  }, [authDeps]);

  const setPreference = <K extends keyof GamePreferences>(
    key: K,
    value: GamePreferences[K],
  ) => {
    setPreferences((prev) => {
      const next = { ...prev, [key]: value };
      if (mounted) {
        try {
          localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
        } catch {
          // storage unavailable
        }
        void savePlayerPreferences({ game: next }, authDeps).catch((error) => {
          console.warn("Could not sync game preferences:", error);
        });
      }
      return next;
    });
  };

  const resetPreferences = () => {
    setPreferences(DEFAULT_PREFERENCES);
    if (mounted) {
      try {
        localStorage.removeItem(STORAGE_KEY);
      } catch {
        // storage unavailable
      }
      void savePlayerPreferences({ game: DEFAULT_PREFERENCES }, authDeps).catch((error) => {
        console.warn("Could not sync reset game preferences:", error);
      });
    }
  };

  const value = {
    preferences,
    setPreference,
    resetPreferences,
  };

  return (
    <GamePreferencesContext.Provider value={value}>
      {children}
    </GamePreferencesContext.Provider>
  );
};

export const useGamePreferences = () => {
  const context = useContext(GamePreferencesContext);
  if (!context) {
    throw new Error(
      "useGamePreferences must be used within a GamePreferencesProvider",
    );
  }
  return context;
};