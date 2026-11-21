"use client";

import { useEffect } from "react";
import { useGamePreferences } from "@/context/GamePreferencesContext";

export interface GameShortcutActions {
  flipBoard?: () => void;
  stepBack?: () => void;
  stepForward?: () => void;
  openChat?: () => void;
  resign?: () => void;
  help?: () => void;
}

const isTypingTarget = (target: EventTarget | null): boolean => {
  const element = target as HTMLElement | null;
  if (!element) return false;
  if (element.isContentEditable) return true;
  const tag = element.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
};

export function useGameShortcuts(
  actions: GameShortcutActions,
  enabled = true,
): void {
  const { preferences } = useGamePreferences();
  const { flipBoard, stepBack, stepForward, openChat, resign, help } = actions;

  useEffect(() => {
    if (!enabled || !preferences.keyboardShortcutsEnabled) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        isTypingTarget(event.target)
      ) {
        return;
      }

      const modalOpen = document.querySelector(
        '[role="dialog"][aria-modal="true"]',
      );
      if (modalOpen && event.key !== "?") return;

      switch (event.key) {
        case "f":
          flipBoard?.();
          break;
        case "ArrowLeft":
          stepBack?.();
          break;
        case "ArrowRight":
          stepForward?.();
          break;
        case "c":
          openChat?.();
          break;
        case "r":
          resign?.();
          break;
        case "?":
          help?.();
          break;
        default:
          return;
      }
      event.preventDefault();
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    enabled,
    preferences.keyboardShortcutsEnabled,
    flipBoard,
    stepBack,
    stepForward,
    openChat,
    resign,
    help,
  ]);
}
