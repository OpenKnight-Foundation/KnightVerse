"use client";
import React, { createContext, useContext, useMemo, useState } from "react";
import {
  DEFAULT_CHESS_VARIANT,
  type ChessVariant,
} from "@/lib/chessVariants";

export type AiPersonality = "aggressive" | "defensive" | "sacrificial";

type MatchmakingContextType = {
  aiPersonality: AiPersonality;
  setAiPersonality: (personality: AiPersonality) => void;
  chessVariant: ChessVariant;
  setChessVariant: (variant: ChessVariant) => void;
};

const MatchmakingContext = createContext<MatchmakingContextType | undefined>(undefined);

export function MatchmakingProvider({ children }: { children: React.ReactNode }) {
  const [aiPersonality, setAiPersonality] = useState<AiPersonality>("aggressive");
  const [chessVariant, setChessVariant] =
    useState<ChessVariant>(DEFAULT_CHESS_VARIANT);
  const value = useMemo(
    () => ({
      aiPersonality,
      setAiPersonality,
      chessVariant,
      setChessVariant,
    }),
    [aiPersonality, chessVariant],
  );

  return (
    <MatchmakingContext.Provider value={value}>
      {children}
    </MatchmakingContext.Provider>
  );
}

export const useMatchmakingContext = () => {
  const ctx = useContext(MatchmakingContext);
  if (!ctx) throw new Error("useMatchmakingContext must be used within MatchmakingProvider");
  return ctx;
};
