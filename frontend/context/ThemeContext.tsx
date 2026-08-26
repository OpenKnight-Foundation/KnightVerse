"use client";

import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  useCallback,
  useMemo,
} from "react";
import { syncThemeWithBackend } from "@/services/themeService";

export type PresetTheme =
  | "emerald"
  | "wood"
  | "obsidian"
  | "neon"
  | "classic_blue"
  | "bordeaux";

export type LegacyTheme = "default" | "cyberpunk" | "classic_wood";

export type BoardTheme = PresetTheme | LegacyTheme | "custom";

export interface ThemeColors {
  light: string;
  dark: string;
  selected: string;
  lastMove: string;
}

export interface PresetThemeInfo {
  id: PresetTheme;
  label: string;
  description: string;
  colors: ThemeColors;
}

export const PRESET_THEMES: Record<PresetTheme, PresetThemeInfo> = {
  emerald: {
    id: "emerald",
    label: "Emerald",
    description: "Curated tournament green with classic soft cream squares",
    colors: {
      light: "#eeeed2",
      dark: "#779954",
      selected: "#baca44",
      lastMove: "#f5f682",
    },
  },
  wood: {
    id: "wood",
    label: "Wood",
    description: "Traditional warm wooden grains and natural finishes",
    colors: {
      light: "#f0d9b5",
      dark: "#b58863",
      selected: "#829769",
      lastMove: "#ced26b",
    },
  },
  obsidian: {
    id: "obsidian",
    label: "Obsidian",
    description: "Sleek dark monochrome high-contrast aesthetic",
    colors: {
      light: "#e0e0e0",
      dark: "#3c3c3c",
      selected: "#708090",
      lastMove: "#556b2f",
    },
  },
  neon: {
    id: "neon",
    label: "Neon",
    description: "Vibrant futuristic glowing synthwave palette",
    colors: {
      light: "#00f0ff",
      dark: "#7400b8",
      selected: "#ff007f",
      lastMove: "#39ff14",
    },
  },
  classic_blue: {
    id: "classic_blue",
    label: "Classic Blue",
    description: "Calm professional blue tournament standard",
    colors: {
      light: "#dee3e6",
      dark: "#8ca2ad",
      selected: "#64b5f6",
      lastMove: "#90caf9",
    },
  },
  bordeaux: {
    id: "bordeaux",
    label: "Bordeaux",
    description: "Rich velvety wine and blush contrast tones",
    colors: {
      light: "#e7cfcf",
      dark: "#802b3e",
      selected: "#c57d56",
      lastMove: "#d3a29d",
    },
  },
};

// Backwards compatibility map
export const LEGACY_THEME_MAP: Record<LegacyTheme, PresetTheme> = {
  default: "emerald",
  classic_wood: "wood",
  cyberpunk: "neon",
};

export const DEFAULT_CUSTOM_PALETTE: ThemeColors = {
  light: "#ffffff",
  dark: "#008e90",
  selected: "#00bcd4",
  lastMove: "#80deea",
};

export const THEME_COLORS: Record<BoardTheme, ThemeColors> = {
  emerald: PRESET_THEMES.emerald.colors,
  wood: PRESET_THEMES.wood.colors,
  obsidian: PRESET_THEMES.obsidian.colors,
  neon: PRESET_THEMES.neon.colors,
  classic_blue: PRESET_THEMES.classic_blue.colors,
  bordeaux: PRESET_THEMES.bordeaux.colors,
  // Backwards compatibility
  default: PRESET_THEMES.emerald.colors,
  classic_wood: PRESET_THEMES.wood.colors,
  cyberpunk: PRESET_THEMES.neon.colors,
  custom: DEFAULT_CUSTOM_PALETTE,
};

// --- Accessibility & Contrast Utilities ---

export function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const cleanHex = hex.replace("#", "").trim();
  if (cleanHex.length === 3) {
    const r = parseInt(cleanHex[0] + cleanHex[0], 16);
    const g = parseInt(cleanHex[1] + cleanHex[1], 16);
    const b = parseInt(cleanHex[2] + cleanHex[2], 16);
    if (isNaN(r) || isNaN(g) || isNaN(b)) return null;
    return { r, g, b };
  }
  if (cleanHex.length === 6) {
    const r = parseInt(cleanHex.substring(0, 2), 16);
    const g = parseInt(cleanHex.substring(2, 4), 16);
    const b = parseInt(cleanHex.substring(4, 6), 16);
    if (isNaN(r) || isNaN(g) || isNaN(b)) return null;
    return { r, g, b };
  }
  return null;
}

export function getRelativeLuminance(r: number, g: number, b: number): number {
  const [rs, gs, bs] = [r, g, b].map((c) => {
    const s = c / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * rs + 0.7152 * gs + 0.0722 * bs;
}

export function calculateContrastRatio(hex1: string, hex2: string): number {
  const rgb1 = hexToRgb(hex1);
  const rgb2 = hexToRgb(hex2);
  if (!rgb1 || !rgb2) return 1;

  const lum1 = getRelativeLuminance(rgb1.r, rgb1.g, rgb1.b);
  const lum2 = getRelativeLuminance(rgb2.r, rgb2.g, rgb2.b);

  const brightest = Math.max(lum1, lum2);
  const darkest = Math.min(lum1, lum2);

  const ratio = (brightest + 0.05) / (darkest + 0.05);
  return Math.round(ratio * 100) / 100;
}

export function getContrastEvaluation(ratio: number): {
  isAccessible: boolean;
  warning: string | null;
  rating: "AAA" | "AA" | "POOR" | "FAIL";
} {
  if (ratio <= 1.1) {
    return {
      isAccessible: false,
      warning: "Square colors are too similar or identical. Minimum contrast required.",
      rating: "FAIL",
    };
  }
  if (ratio < 2.0) {
    return {
      isAccessible: false,
      warning: `Low contrast warning (${ratio}:1). Light and dark squares will be difficult to distinguish during fast play.`,
      rating: "POOR",
    };
  }
  if (ratio < 4.5) {
    return {
      isAccessible: true,
      warning: null,
      rating: "AA",
    };
  }
  return {
    isAccessible: true,
    warning: null,
    rating: "AAA",
  };
}

interface ThemeContextProps {
  boardTheme: BoardTheme;
  setBoardTheme: (theme: BoardTheme) => void;
  customPalette: ThemeColors;
  setCustomPalette: (palette: Partial<ThemeColors>) => void;
  colors: ThemeColors;
  resetTheme: () => void;
  contrastRatio: number;
  contrastWarning: string | null;
  isSynced: boolean;
}

const ThemeContext = createContext<ThemeContextProps | undefined>(undefined);

export const BoardThemeProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [boardTheme, setBoardThemeState] = useState<BoardTheme>("emerald");
  const [customPalette, setCustomPaletteState] = useState<ThemeColors>(
    DEFAULT_CUSTOM_PALETTE
  );
  const [mounted, setMounted] = useState(false);
  const [isSynced, setIsSynced] = useState(false);

  // Apply CSS custom properties dynamically to prevent layout reflows
  const applyCssVariables = useCallback((themeColors: ThemeColors) => {
    if (typeof document !== "undefined") {
      const root = document.documentElement;
      root.style.setProperty("--board-light", themeColors.light);
      root.style.setProperty("--board-dark", themeColors.dark);
      root.style.setProperty("--board-selected", themeColors.selected);
      root.style.setProperty("--board-last-move", themeColors.lastMove);
    }
  }, []);

  // Compute active colors
  const activeColors: ThemeColors = useMemo(() => {
    if (boardTheme === "custom") {
      return customPalette;
    }
    if (PRESET_THEMES[boardTheme as PresetTheme]) {
      return PRESET_THEMES[boardTheme as PresetTheme].colors;
    }
    if (LEGACY_THEME_MAP[boardTheme as LegacyTheme]) {
      const mapped = LEGACY_THEME_MAP[boardTheme as LegacyTheme];
      return PRESET_THEMES[mapped].colors;
    }
    return PRESET_THEMES.emerald.colors;
  }, [boardTheme, customPalette]);

  // Calculate contrast ratio & warning for active colors
  const contrastRatio = useMemo(
    () => calculateContrastRatio(activeColors.light, activeColors.dark),
    [activeColors.light, activeColors.dark]
  );

  const contrastWarning = useMemo(
    () => getContrastEvaluation(contrastRatio).warning,
    [contrastRatio]
  );

  // Load from localStorage on mount
  useEffect(() => {
    try {
      const savedTheme = localStorage.getItem("knightverse_board_theme") as BoardTheme | null;
      const savedPalette = localStorage.getItem("knightverse_custom_board_palette");

      let initialPalette = DEFAULT_CUSTOM_PALETTE;
      if (savedPalette) {
        try {
          const parsed = JSON.parse(savedPalette);
          if (parsed && parsed.light && parsed.dark) {
            initialPalette = {
              light: parsed.light,
              dark: parsed.dark,
              selected: parsed.selected || DEFAULT_CUSTOM_PALETTE.selected,
              lastMove: parsed.lastMove || DEFAULT_CUSTOM_PALETTE.lastMove,
            };
            setCustomPaletteState(initialPalette);
          }
        } catch {
          // ignore corrupted palette
        }
      }

      if (savedTheme) {
        if (savedTheme === "custom") {
          setBoardThemeState("custom");
          applyCssVariables(initialPalette);
        } else if (PRESET_THEMES[savedTheme as PresetTheme]) {
          setBoardThemeState(savedTheme);
          applyCssVariables(PRESET_THEMES[savedTheme as PresetTheme].colors);
        } else if (LEGACY_THEME_MAP[savedTheme as LegacyTheme]) {
          const normalized = LEGACY_THEME_MAP[savedTheme as LegacyTheme];
          setBoardThemeState(normalized);
          applyCssVariables(PRESET_THEMES[normalized].colors);
        } else {
          setBoardThemeState("emerald");
          applyCssVariables(PRESET_THEMES.emerald.colors);
        }
      } else {
        applyCssVariables(PRESET_THEMES.emerald.colors);
      }
    } catch {
      // localStorage not accessible
    }
    setMounted(true);
  }, [applyCssVariables]);

  // Update CSS variables when active colors change
  useEffect(() => {
    if (mounted) {
      applyCssVariables(activeColors);
    }
  }, [activeColors, mounted, applyCssVariables]);

  const setBoardTheme = useCallback(
    (theme: BoardTheme) => {
      // Map legacy theme to modern preset
      const resolvedTheme: BoardTheme =
        theme in LEGACY_THEME_MAP
          ? LEGACY_THEME_MAP[theme as LegacyTheme]
          : theme;

      setBoardThemeState(resolvedTheme);

      if (typeof window !== "undefined") {
        try {
          localStorage.setItem("knightverse_board_theme", resolvedTheme);
        } catch {
          // storage quota / disabled
        }
      }

      // Sync with profile backend API if logged in
      syncThemeWithBackend({
        boardTheme: resolvedTheme,
        customPalette: resolvedTheme === "custom" ? customPalette : undefined,
      }).then((synced) => {
        setIsSynced(synced);
      });
    },
    [customPalette]
  );

  const setCustomPalette = useCallback(
    (paletteUpdates: Partial<ThemeColors>) => {
      setCustomPaletteState((prev) => {
        const updated: ThemeColors = {
          ...prev,
          ...paletteUpdates,
        };

        if (typeof window !== "undefined") {
          try {
            localStorage.setItem(
              "knightverse_custom_board_palette",
              JSON.stringify(updated)
            );
            localStorage.setItem("knightverse_board_theme", "custom");
          } catch {
            // storage error
          }
        }

        // Also switch active theme to custom if updating palette
        setBoardThemeState("custom");

        syncThemeWithBackend({
          boardTheme: "custom",
          customPalette: updated,
        }).then((synced) => {
          setIsSynced(synced);
        });

        return updated;
      });
    },
    []
  );

  const resetTheme = useCallback(() => {
    setBoardThemeState("emerald");
    setCustomPaletteState(DEFAULT_CUSTOM_PALETTE);
    if (typeof window !== "undefined") {
      try {
        localStorage.setItem("knightverse_board_theme", "emerald");
        localStorage.removeItem("knightverse_custom_board_palette");
      } catch {
        // storage error
      }
    }
    syncThemeWithBackend({
      boardTheme: "emerald",
    }).then((synced) => {
      setIsSynced(synced);
    });
  }, []);

  const value: ThemeContextProps = {
    boardTheme: mounted ? boardTheme : "emerald",
    setBoardTheme,
    customPalette,
    setCustomPalette,
    colors: activeColors,
    resetTheme,
    contrastRatio,
    contrastWarning,
    isSynced,
  };

  return (
    <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>
  );
};

export const useBoardTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error("useBoardTheme must be used within a ThemeProvider");
  }
  return context;
};
