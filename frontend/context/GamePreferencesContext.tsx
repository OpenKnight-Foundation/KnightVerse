"use client";

import React, { createContext, useContext, useState, useEffect } from "react";

export type PieceInputMethod = "drag" | "click" | "both";
export type AutoQueenMode = "always" | "prompt" | "premoves";
export type LegalMoveDots = "enabled" | "disabled";
export type BoardCoordinates = "inside" | "outside" | "hidden";
export type PieceSet = "neo" | "staunton" | "alpha" | "medieval" | "cyberpunk";

export interface GamePreferences {
  pieceInputMethod: PieceInputMethod;
  autoQueen: AutoQueenMode;
  showLegalMoveDots: LegalMoveDots;
  confirmMoveCorrespondence: boolean;
  boardCoordinates: BoardCoordinates;
  pieceSet: PieceSet;
}

export const DEFAULT_PREFERENCES: GamePreferences = {
  pieceInputMethod: "both",
  autoQueen: "always",
  showLegalMoveDots: "enabled",
  confirmMoveCorrespondence: false,
  boardCoordinates: "inside",
  pieceSet: "neo",
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

  useEffect(() => {
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
  }, []);

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