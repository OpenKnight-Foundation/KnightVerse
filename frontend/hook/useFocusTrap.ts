"use client";

import { useEffect, useRef, useCallback } from "react";

// ---------------------------------------------------------------------------
// Focusable element selector (WCAG 2.1 interactive elements)
// ---------------------------------------------------------------------------

const FOCUSABLE_SELECTORS = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
  "details > summary",
].join(", ");

function getFocusableElements(container: HTMLElement): HTMLElement[] {
  return Array.from(
    container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTORS),
  ).filter(
    (el) =>
      !el.hasAttribute("disabled") &&
      el.getAttribute("aria-hidden") !== "true",
  );
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export interface UseFocusTrapOptions {
  /** Whether the trap is currently active (e.g. modal is open). */
  active: boolean;
  /**
   * When true, focus will be restored to the element that held focus before
   * the trap was activated.  Defaults to `true`.
   */
  restoreFocus?: boolean;
  /**
   * Optional element to focus on trap activation.
   * If not provided, the first focusable child is focused.
   */
  initialFocusRef?: React.RefObject<HTMLElement | null>;
}

/**
 * useFocusTrap
 *
 * Constrains keyboard focus within a container element when `active` is true.
 *
 * - Focuses the first focusable child (or `initialFocusRef`) on activation.
 * - Traps Tab / Shift+Tab within the container.
 * - Closes on Escape (calls `onEscape` if provided).
 * - Restores focus to the previously focused element when deactivated.
 *
 * Usage:
 * ```tsx
 * const ref = useFocusTrap({ active: isOpen, onEscape: onClose });
 * // ...
 * <div ref={ref} ...>...</div>
 * ```
 */
export function useFocusTrap(
  options: UseFocusTrapOptions & { onEscape?: () => void },
): React.RefObject<HTMLDivElement | null> {
  const { active, restoreFocus = true, initialFocusRef, onEscape } = options;
  const containerRef = useRef<HTMLDivElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  // ── Activation / deactivation ─────────────────────────────────────────────
  useEffect(() => {
    if (!active) {
      // Restore focus when trap is released
      if (restoreFocus && previousFocusRef.current) {
        previousFocusRef.current.focus();
        previousFocusRef.current = null;
      }
      return;
    }

    // Save current focus so we can restore it later
    previousFocusRef.current = document.activeElement as HTMLElement | null;

    // Move focus into the modal
    const container = containerRef.current;
    if (!container) return;

    // Use a short setTimeout(0) so focus runs after the paint microtask
    // while still being synchronous enough for jsdom in tests.
    const id = setTimeout(() => {
      const target = initialFocusRef?.current;
      if (target) {
        target.focus();
      } else {
        const focusable = getFocusableElements(container);
        if (focusable.length > 0) {
          focusable[0].focus();
        } else {
          // Fall back to focusing the container itself so keyboard still works
          container.setAttribute("tabindex", "-1");
          container.focus();
        }
      }
    }, 0);

    return () => clearTimeout(id);
  }, [active, initialFocusRef, restoreFocus]);

  // ── Keyboard trap ─────────────────────────────────────────────────────────
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!active) return;
      const container = containerRef.current;
      if (!container) return;

      if (e.key === "Escape") {
        e.stopPropagation();
        onEscape?.();
        return;
      }

      if (e.key !== "Tab") return;

      const focusable = getFocusableElements(container);
      if (focusable.length === 0) {
        e.preventDefault();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const currentFocus = document.activeElement as HTMLElement;

      if (e.shiftKey) {
        // Shift+Tab: if focus is on the first element, wrap to last
        if (currentFocus === first || !container.contains(currentFocus)) {
          e.preventDefault();
          last.focus();
        }
      } else {
        // Tab: if focus is on the last element, wrap to first
        if (currentFocus === last || !container.contains(currentFocus)) {
          e.preventDefault();
          first.focus();
        }
      }
    },
    [active, onEscape],
  );

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown, true);
    return () => document.removeEventListener("keydown", handleKeyDown, true);
  }, [handleKeyDown]);

  return containerRef;
}

export default useFocusTrap;
