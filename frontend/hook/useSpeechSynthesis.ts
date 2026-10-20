"use client";

import { useCallback, useEffect, useRef, useState } from "react";

export interface SpeakOptions {
  volume?: number; // 0-100
  rate?: number; // 0.5-2
  lang?: string;
}

/**
 * FE-52: thin wrapper around window.speechSynthesis (Web Speech API TTS).
 * No-op when unsupported (SSR / non-browser / no voices).
 */
export function useSpeechSynthesis() {
  const [supported] = useState(
    () => typeof window !== "undefined" && "speechSynthesis" in window,
  );
  const lastUtterance = useRef<SpeechSynthesisUtterance | null>(null);

  const cancel = useCallback(() => {
    if (typeof window !== "undefined" && "speechSynthesis" in window) {
      window.speechSynthesis.cancel();
    }
  }, []);

  useEffect(() => cancel, [cancel]);

  const speak = useCallback(
    (text: string, opts: SpeakOptions = {}) => {
      if (!text || typeof window === "undefined" || !("speechSynthesis" in window)) {
        return false;
      }
      try {
        window.speechSynthesis.cancel();
        const utterance = new SpeechSynthesisUtterance(text);
        utterance.volume = Math.min(1, Math.max(0, (opts.volume ?? 80) / 100));
        utterance.rate = Math.min(2, Math.max(0.5, opts.rate ?? 1));
        if (opts.lang) utterance.lang = opts.lang;
        lastUtterance.current = utterance;
        window.speechSynthesis.speak(utterance);
        return true;
      } catch {
        return false;
      }
    },
    [],
  );

  return { supported, speak, cancel };
}

export default useSpeechSynthesis;
