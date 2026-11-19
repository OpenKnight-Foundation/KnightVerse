"use client";

import { useEffect, useRef } from "react";

interface ShortcutsHelpDialogProps {
  open: boolean;
  onClose: () => void;
}

const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: "F", action: "Flip the board" },
  { keys: "← / →", action: "Step one move back / forward" },
  { keys: "C", action: "Open game chat" },
  { keys: "R", action: "Resign with confirmation (live games only)" },
  { keys: "?", action: "Show or hide this help" },
  { keys: "Esc", action: "Close dialogs" },
];

export function ShortcutsHelpDialog({ open, onClose }: ShortcutsHelpDialogProps) {
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[120] flex items-center justify-center bg-black/70 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="shortcuts-help-title"
    >
      <div className="w-full max-w-md rounded-xl border border-teal-500/40 bg-gray-900 p-5 text-white shadow-2xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 id="shortcuts-help-title" className="text-lg font-bold text-teal-300">
            Keyboard Shortcuts
          </h2>
          <button
            ref={closeRef}
            type="button"
            onClick={onClose}
            aria-label="Close keyboard shortcuts help"
            className="rounded-lg px-2 py-1 text-gray-400 hover:bg-gray-800 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
          >
            ✕
          </button>
        </div>
        <dl className="space-y-2">
          {SHORTCUTS.map((shortcut) => (
            <div key={shortcut.keys} className="flex items-center justify-between gap-4">
              <dt>
                <kbd className="rounded bg-gray-800 px-2 py-1 font-mono text-xs text-teal-200">
                  {shortcut.keys}
                </kbd>
              </dt>
              <dd className="text-sm text-gray-300">{shortcut.action}</dd>
            </div>
          ))}
        </dl>
        <p className="mt-4 text-xs text-gray-500">
          Shortcuts are ignored while typing and can be disabled in Game Preferences.
        </p>
      </div>
    </div>
  );
}

export default ShortcutsHelpDialog;
