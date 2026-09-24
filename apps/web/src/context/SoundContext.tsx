import React, { createContext, useContext, useEffect, useMemo, useState, useCallback } from 'react';
import { soundService, SOUND_EVENTS, SoundPack, SoundEvent } from '../services/soundService';

interface SoundContextValue {
  selectedPack: SoundPack;
  masterVolume: number;
  eventVolumes: Record<SoundEvent, number>;
  muted: boolean;
  setSelectedPack: (pack: SoundPack) => void;
  setMasterVolume: (v: number) => void;
  setEventVolume: (event: SoundEvent, v: number) => void;
  toggleMuted: () => void;
  play: (event: SoundEvent) => void;
  preview: (pack: SoundPack, event: SoundEvent) => void;
  setLowTimeAlarm: (active: boolean) => void;
}

const SoundContext = createContext<SoundContextValue | undefined>(undefined);

const STORAGE_KEY = 'chess-sound-settings';

interface StoredSettings {
  selectedPack: SoundPack;
  masterVolume: number;
  eventVolumes: Partial<Record<SoundEvent, number>>;
  muted: boolean;
}

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, v));
}

function loadSettings(): StoredSettings {
  const defaults: StoredSettings = {
    selectedPack: 'classic-wood',
    masterVolume: 0.7,
    eventVolumes: { move: 0.8, capture: 0.8, check: 0.8, castle: 0.8, promote: 0.8, low_time: 0.6, victory: 1, defeat: 1 },
    muted: false,
  };
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        selectedPack: parsed.selectedPack ?? defaults.selectedPack,
        masterVolume: parsed.masterVolume ?? defaults.masterVolume,
        eventVolumes: { ...defaults.eventVolumes, ...parsed.eventVolumes },
        muted: parsed.muted ?? defaults.muted,
      };
    }
  } catch { /* ignore */ }
  return defaults;
}

export const SoundProvider: React.FC< { children: React.ReactNode }> = ({ children }) => {
  const [settings, setSettings] = useState<StoredSettings>(loadSettings);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  useEffect(() => {
    soundService.setPack(settings.selectedPack);
    soundService.setMasterVolume(settings.muted ? 0 : settings.masterVolume);
    SOUND_EVENTS.forEach(event => {
      soundService.setEventVolume(event, settings.eventVolumes[event] ?? 1);
    });
    soundService.preloadPack(settings.selectedPack);
  }, [settings]);

  const play = useCallback((event: SoundEvent) => {
    if (settings.muted) return;
    soundService.play(event);
  }, [settings.muted]);

  const preview = useCallback((pack: SoundPack, event: SoundEvent) => {
    if (settings.muted) return;
    soundService.preview(pack, event);
  }, [settings.muted]);

  const setSelectedPack = useCallback((pack: SoundPack) => {
    setSettings(s => { ...s, selectedPack: pack });
  }, []);

  const setMasterVolume = useCallback((v: number) => {
    setSettings(s => { ...s, masterVolume: clamp01(v) });
  }, []);

  const setEventVolume = useCallback((event: SoundEvent, v: number) => {
    setSettings(s => ({ ...s, eventVolumes: { ...s.eventVolumes, [event]: clamp01(v) } });
  }, []);

  const toggleMuted = useCallback(() => {
    setSettings(s => ({ ...s, muted: !s.muted }));
  }, []);

  const setLowTimeAlarm = useCallback((active: boolean) => {
    soundService.setLowTimeAlarm(active);
  }, []);

  const value = useMemo<SoundContextValue>(() => ({
    selectedPack: settings.selectedPack,
    masterVolume: settings.masterVolume,
    eventVolumes: {
      move: settings.eventVolumes['move'] ?? 0.8,
      capture: settings.eventVolumes['capture'] ?? 0.8,
      check: settings.eventVolumes['check'] ?? 0.8,
      castle: settings.eventVolumes['castle'] ?? 0.8,
      promote: settings.eventVolumes['promote'] ?? 0.8,
      low_time: settings.eventVolumes['low_time'] ?? 0.6,
      victory: settings.eventVolumes['victory'] ?? 1,
      defeat: settings.eventVolumes['defeat'] ?? 1,
    },
    muted: settings.muted,
    setSelectedPack,
    setMasterVolume,
    setEventVolume,
    toggleMuted,
    play,
    preview,
    setLowTimeAlarm,
  }), [settings, setSelectedPack, setMasterVolume, setEventVolume, toggleMuted, play, preview, setLowTimeAlarm]);

  return <SoundContext.Provider value={value}>{children}</SoundContext.Provider>;
};

export function useSound() {
  const ctx = useContext(SoundContext);
  if (!ctx) throw new Error('useSound must be used within SoundProvider');
  return ctx;
}
