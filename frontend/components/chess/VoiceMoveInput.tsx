"use client";

import React, { useCallback, useEffect, useRef, useState } from "react";
import { parseVoiceToSan } from "@/lib/parseVoiceMove";

interface VoiceMoveInputProps {
  /** Submit a SAN move. Returns true if legal and accepted. */
  onSubmitMove: (san: string) => boolean;
  isGameActive: boolean;
  isMyTurn: boolean;
  /** When false, only the keyboard fallback hint is shown. */
  enabled?: boolean;
}

type RecognitionInstance = {
  lang: string;
  interimResults: boolean;
  continuous: boolean;
  start: () => void;
  stop: () => void;
  onresult: ((e: { results: ArrayLike<ArrayLike<{ transcript: string }>> }) => void) | null;
  onerror: ((e: { error?: string }) => void) | null;
  onend: (() => void) | null;
};

function getRecognitionConstructor(): (new () => RecognitionInstance) | null {
  if (typeof window === "undefined") return null;
  const w = window as unknown as Record<string, unknown>;
  return (
    (w.SpeechRecognition as new () => RecognitionInstance) ??
    (w.webkitSpeechRecognition as new () => RecognitionInstance) ??
    null
  );
}

/**
 * FE-52: voice move input via Web Speech API SpeechRecognition.
 * Falls back to manual keyboard input (KeyboardMoveInput) when unsupported.
 */
export function VoiceMoveInput({
  onSubmitMove,
  isGameActive,
  isMyTurn,
  enabled = true,
}: VoiceMoveInputProps) {
  const [supported, setSupported] = useState(false);
  const [listening, setListening] = useState(false);
  const [transcript, setTranscript] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const recognitionRef = useRef<RecognitionInstance | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setSupported(getRecognitionConstructor() !== null);
  }, []);

  useEffect(() => {
    return () => {
      try {
        recognitionRef.current?.stop();
      } catch {
        // ignore
      }
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, []);

  const submitTranscript = useCallback(
    (raw: string) => {
      const san = parseVoiceToSan(raw);
      if (!san) {
        setError(`Could not understand "${raw}". Try e.g. "Knight f3".`);
        timeoutRef.current = setTimeout(() => setError(null), 4000);
        return;
      }
      const accepted = onSubmitMove(san);
      if (accepted) {
        setTranscript("");
        setSuccess(`Voice move accepted: ${san}`);
        timeoutRef.current = setTimeout(() => setSuccess(null), 2500);
      } else {
        setError(`Illegal move: ${san}. Try again.`);
        timeoutRef.current = setTimeout(() => setError(null), 4000);
      }
    },
    [onSubmitMove],
  );

  const toggleListening = useCallback(() => {
    if (listening) {
      try {
        recognitionRef.current?.stop();
      } catch {
        // ignore
      }
      setListening(false);
      return;
    }
    const Ctor = getRecognitionConstructor();
    if (!Ctor) {
      setError("Voice input is not supported in this browser. Use keyboard input.");
      return;
    }
    setError(null);
    try {
      const recognition = new Ctor();
      recognition.lang = "en-US";
      recognition.interimResults = true;
      recognition.continuous = false;
      recognition.onresult = (e) => {
        const last = e.results[e.results.length - 1];
        const text = last?.[0]?.transcript ?? "";
        setTranscript(text);
        // Only auto-submit final results; interim updates just display.
        // Treat missing isFinal as final (defensive for mocks).
        const isFinal = (last as unknown as { isFinal?: boolean })?.isFinal;
        if (text.trim() && isFinal !== false) submitTranscript(text);
      };
      recognition.onerror = (e) => {
        setError(
          e?.error === "not-allowed"
            ? "Microphone blocked. Allow access to use voice moves."
            : "Voice recognition error. Try again or type your move.",
        );
        setListening(false);
      };
      recognition.onend = () => setListening(false);
      recognitionRef.current = recognition;
      recognition.start();
      setListening(true);
    } catch {
      setError("Could not start voice input. Use keyboard input.");
    }
  }, [listening, submitTranscript]);

  if (!isGameActive || !enabled) return null;

  return (
    <div className="w-full" aria-label="Voice move input">
      <div className="flex items-center gap-2">
        {supported ? (
          <button
            type="button"
            onClick={toggleListening}
            disabled={!isMyTurn}
            aria-label={listening ? "Stop voice input" : "Start voice input"}
            aria-pressed={listening}
            className={`px-4 py-2 rounded-lg text-sm font-medium transition-all focus:outline-none focus:ring-2 focus:ring-blue-500/50 disabled:opacity-50 disabled:cursor-not-allowed ${
              listening
                ? "bg-red-600 hover:bg-red-500 text-white"
                : "bg-purple-600 hover:bg-purple-500 text-white"
            }`}
          >
            {listening ? "● Listening… (tap to stop)" : "🎤 Voice move"}
          </button>
        ) : (
          <p className="text-xs text-gray-500">
            Voice input not supported here — type your move below (e.g. e4, Nf3, O-O).
          </p>
        )}
        {transcript && (
          <p className="text-xs text-gray-300 font-mono truncate" aria-hidden="true">
            “{transcript}”
          </p>
        )}
      </div>
      <div role="status" aria-live="polite" aria-atomic="true" className="sr-only">
        {success ?? transcript}
      </div>
      <div role="alert" aria-live="assertive" aria-atomic="true" className="sr-only">
        {error}
      </div>
      {error && (
        <p className="mt-1 text-xs text-red-400 font-medium" aria-hidden="true">
          {error}
        </p>
      )}
      {success && (
        <p className="mt-1 text-xs text-emerald-400" aria-hidden="true">
          {success}
        </p>
      )}
    </div>
  );
}

export default VoiceMoveInput;
