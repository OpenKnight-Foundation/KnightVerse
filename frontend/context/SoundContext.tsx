"use client";

import React, { createContext, useContext, useState, useEffect } from 'react';

import { debugLog } from '@/lib/debugLogger';

type SoundPack = 'standard' | 'arcade' | 'minimalist';

interface SoundContextType {
  volume: number;
  setVolume: (v: number) => void;
  isMuted: boolean;
  setIsMuted: (m: boolean) => void;
  soundPack: SoundPack;
  setSoundPack: (s: SoundPack) => void;
  playSound: (event: 'move' | 'capture' | 'check') => void;
}

const SoundContext = createContext<SoundContextType | undefined>(undefined);

export function SoundProvider({ children }: { children: React.ReactNode }) {
  const [volume, setVolume] = useState<number>(50);
  const [isMuted, setIsMuted] = useState<boolean>(false);
  const [soundPack, setSoundPack] = useState<SoundPack>('standard');

  useEffect(() => {
    // Mock pre-loading audio assets
    debugLog(`Pre-loading audio assets for sound pack: ${soundPack}`);
  }, [soundPack]);

  const playSound = (event: 'move' | 'capture' | 'check') => {
    if (isMuted) return;
    debugLog(`Playing ${event} sound from ${soundPack} pack at volume ${volume}`);
    // In a real implementation, we would play the actual audio file here
  };

  return (
    <SoundContext.Provider value={{ volume, setVolume, isMuted, setIsMuted, soundPack, setSoundPack, playSound }}>
      {children}
    </SoundContext.Provider>
  );
}

export function useSound() {
  const context = useContext(SoundContext);
  if (context === undefined) {
    throw new Error('useSound must be used within a SoundProvider');
  }
  return context;
}
