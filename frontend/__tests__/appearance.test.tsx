import React from "react";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import AppearanceSettings from "@/components/AppearanceSettings";
import {
  BoardThemeProvider,
  useBoardTheme,
  PRESET_THEMES,
  calculateContrastRatio,
  getContrastEvaluation,
  hexToRgb,
  LEGACY_THEME_MAP,
} from "@/context/ThemeContext";
import {
  syncThemeWithBackend,
  fetchThemePreferencesFromBackend,
} from "@/services/themeService";

describe("Theme Studio & Appearance Unit Tests", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    localStorage.clear();
  });

  describe("1. Preset Themes & Configuration", () => {
    it("defines all 6 required preset themes with complete color palettes", () => {
      const expectedPresets = [
        "emerald",
        "wood",
        "obsidian",
        "neon",
        "classic_blue",
        "bordeaux",
      ] as const;

      expectedPresets.forEach((presetKey) => {
        const preset = PRESET_THEMES[presetKey];
        expect(preset).toBeDefined();
        expect(preset.id).toBe(presetKey);
        expect(preset.label).toBeTruthy();
        expect(preset.description).toBeTruthy();
        expect(preset.colors.light).toMatch(/^#[0-9A-Fa-f]{6}$/);
        expect(preset.colors.dark).toMatch(/^#[0-9A-Fa-f]{6}$/);
        expect(preset.colors.selected).toMatch(/^#[0-9A-Fa-f]{6}$/);
        expect(preset.colors.lastMove).toMatch(/^#[0-9A-Fa-f]{6}$/);
      });
    });

    it("correctly maps legacy theme names for backwards compatibility", () => {
      expect(LEGACY_THEME_MAP.default).toBe("emerald");
      expect(LEGACY_THEME_MAP.classic_wood).toBe("wood");
      expect(LEGACY_THEME_MAP.cyberpunk).toBe("neon");
    });
  });

  describe("2. Contrast Ratio & Accessibility Calculations", () => {
    it("converts hex strings to RGB correctly", () => {
      expect(hexToRgb("#ffffff")).toEqual({ r: 255, g: 255, b: 255 });
      expect(hexToRgb("#000000")).toEqual({ r: 0, g: 0, b: 0 });
      expect(hexToRgb("fff")).toEqual({ r: 255, g: 255, b: 255 });
      expect(hexToRgb("invalid")).toBeNull();
    });

    it("calculates 21:1 contrast ratio between pure white and pure black", () => {
      const ratio = calculateContrastRatio("#ffffff", "#000000");
      expect(ratio).toBe(21);
    });

    it("calculates 1:1 contrast ratio for identical colors", () => {
      const ratio = calculateContrastRatio("#779954", "#779954");
      expect(ratio).toBe(1);
    });

    it("evaluates high contrast ratio as accessible with AAA rating", () => {
      const evalResult = getContrastEvaluation(7.5);
      expect(evalResult.isAccessible).toBe(true);
      expect(evalResult.rating).toBe("AAA");
      expect(evalResult.warning).toBeNull();
    });

    it("evaluates low contrast ratio with a warning banner", () => {
      const evalResult = getContrastEvaluation(1.6);
      expect(evalResult.isAccessible).toBe(false);
      expect(evalResult.rating).toBe("POOR");
      expect(evalResult.warning).toContain("Low contrast warning");
    });

    it("evaluates identical colors as FAIL with appropriate warning", () => {
      const evalResult = getContrastEvaluation(1.0);
      expect(evalResult.isAccessible).toBe(false);
      expect(evalResult.rating).toBe("FAIL");
      expect(evalResult.warning).toContain("Square colors are too similar or identical");
    });
  });

  describe("3. ThemeContext State & Persistence", () => {
    const TestConsumer = () => {
      const {
        boardTheme,
        setBoardTheme,
        colors,
        customPalette,
        setCustomPalette,
        resetTheme,
        contrastRatio,
      } = useBoardTheme();

      return (
        <div>
          <span data-testid="theme-id">{boardTheme}</span>
          <span data-testid="color-light">{colors.light}</span>
          <span data-testid="color-dark">{colors.dark}</span>
          <span data-testid="contrast">{contrastRatio}</span>
          <button onClick={() => setBoardTheme("wood")}>Set Wood</button>
          <button onClick={() => setBoardTheme("obsidian")}>Set Obsidian</button>
          <button
            onClick={() =>
              setCustomPalette({
                light: "#ffffff",
                dark: "#123456",
              })
            }
          >
            Set Custom
          </button>
          <button onClick={resetTheme}>Reset</button>
        </div>
      );
    };

    it("initializes with default emerald theme", () => {
      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );

      expect(screen.getByTestId("theme-id").textContent).toBe("emerald");
      expect(screen.getByTestId("color-light").textContent).toBe(
        PRESET_THEMES.emerald.colors.light
      );
      expect(screen.getByTestId("color-dark").textContent).toBe(
        PRESET_THEMES.emerald.colors.dark
      );
    });

    it("switches presets and persists to localStorage", () => {
      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );

      fireEvent.click(screen.getByText("Set Wood"));

      expect(screen.getByTestId("theme-id").textContent).toBe("wood");
      expect(screen.getByTestId("color-light").textContent).toBe(
        PRESET_THEMES.wood.colors.light
      );
      expect(localStorage.getItem("knightverse_board_theme")).toBe("wood");
    });

    it("applies custom palette and updates localStorage", () => {
      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );

      fireEvent.click(screen.getByText("Set Custom"));

      expect(screen.getByTestId("theme-id").textContent).toBe("custom");
      expect(screen.getByTestId("color-light").textContent).toBe("#ffffff");
      expect(screen.getByTestId("color-dark").textContent).toBe("#123456");
      expect(localStorage.getItem("knightverse_board_theme")).toBe("custom");
      expect(localStorage.getItem("knightverse_custom_board_palette")).toContain(
        "#123456"
      );
    });

    it("resets theme to default emerald and clears custom palette from localStorage", () => {
      localStorage.setItem("knightverse_board_theme", "custom");
      localStorage.setItem(
        "knightverse_custom_board_palette",
        JSON.stringify({ light: "#ffffff", dark: "#000000" })
      );

      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );

      fireEvent.click(screen.getByText("Reset"));

      expect(screen.getByTestId("theme-id").textContent).toBe("emerald");
      expect(localStorage.getItem("knightverse_board_theme")).toBe("emerald");
      expect(localStorage.getItem("knightverse_custom_board_palette")).toBeNull();
    });

    it("restores previously saved preset on mount", () => {
      localStorage.setItem("knightverse_board_theme", "bordeaux");

      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );

      expect(screen.getByTestId("theme-id").textContent).toBe("bordeaux");
      expect(screen.getByTestId("color-dark").textContent).toBe(
        PRESET_THEMES.bordeaux.colors.dark
      );
    });

    it("updates CSS variables on document.documentElement", () => {
      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );

      fireEvent.click(screen.getByText("Set Obsidian"));

      expect(
        document.documentElement.style.getPropertyValue("--board-dark")
      ).toBe(PRESET_THEMES.obsidian.colors.dark);
      expect(
        document.documentElement.style.getPropertyValue("--board-light")
      ).toBe(PRESET_THEMES.obsidian.colors.light);
    });
  });

  describe("4. AppearanceSettings Component UI & Interactions", () => {
    it("renders Theme Studio header, preset buttons, and live preview", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );

      expect(screen.getByText("Chessboard Theme Studio")).toBeInTheDocument();
      expect(screen.getByText("Curated Presets")).toBeInTheDocument();
      expect(screen.getByText("Custom Palette")).toBeInTheDocument();
      expect(screen.getByText("Live Interactive Preview")).toBeInTheDocument();

      // Check all 6 presets are rendered
      expect(screen.getByText("Emerald")).toBeInTheDocument();
      expect(screen.getByText("Wood")).toBeInTheDocument();
      expect(screen.getByText("Obsidian")).toBeInTheDocument();
      expect(screen.getByText("Neon")).toBeInTheDocument();
      expect(screen.getByText("Classic Blue")).toBeInTheDocument();
      expect(screen.getByText("Bordeaux")).toBeInTheDocument();
    });

    it("allows switching presets by clicking preset chips", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );

      const neonButton = screen.getByRole("radio", { name: /neon/i });
      fireEvent.click(neonButton);

      expect(neonButton).toHaveAttribute("aria-checked", "true");
      expect(localStorage.getItem("knightverse_board_theme")).toBe("neon");
    });

    it("switches to custom palette tab and renders color pickers", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );

      const customTab = screen.getByRole("button", { name: /custom palette/i });
      fireEvent.click(customTab);

      expect(screen.getByLabelText(/light square color/i)).toBeInTheDocument();
      expect(screen.getByLabelText(/dark square color/i)).toBeInTheDocument();
      expect(
        screen.getByLabelText(/selected square highlight color/i)
      ).toBeInTheDocument();
      expect(
        screen.getByLabelText(/last move highlight color/i)
      ).toBeInTheDocument();
      expect(screen.getByText(/board contrast ratio/i)).toBeInTheDocument();
    });

    it("allows modifying custom colors and displays live contrast ratio", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );

      fireEvent.click(screen.getByRole("button", { name: /custom palette/i }));

      const lightInput = screen.getByLabelText(/light square color/i);
      fireEvent.change(lightInput, { target: { value: "#ffffff" } });

      const darkInput = screen.getByLabelText(/dark square color/i);
      fireEvent.change(darkInput, { target: { value: "#000000" } });

      expect(screen.getByText("21:1")).toBeInTheDocument();
      expect(screen.getByText("WCAG AAA")).toBeInTheDocument();
    });

    it("displays warning when identical colors are chosen", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );

      fireEvent.click(screen.getByRole("button", { name: /custom palette/i }));

      const lightInput = screen.getByLabelText(/light square color/i);
      const darkInput = screen.getByLabelText(/dark square color/i);

      fireEvent.change(lightInput, { target: { value: "#555555" } });
      fireEvent.change(darkInput, { target: { value: "#555555" } });

      expect(
        screen.getByText(/Square colors are too similar or identical/i)
      ).toBeInTheDocument();
    });

    it("interacts with live preview board squares when clicked", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );

      const previewSquare = screen.getByLabelText("Square e4");
      expect(previewSquare).toBeInTheDocument();

      fireEvent.click(previewSquare);
      // Clicking toggles selection on preview board
    });
  });

  describe("5. Backend Profile Sync Service", () => {
    it("returns false if no auth token is available", async () => {
      const result = await syncThemeWithBackend({ boardTheme: "emerald" });
      expect(result).toBe(false);
    });

    it("attempts fetch when auth token is provided and handles success", async () => {
      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ success: true }),
      } as Response);

      const result = await syncThemeWithBackend(
        { boardTheme: "wood" },
        "mock_token_123"
      );
      expect(result).toBe(true);
      expect(global.fetch).toHaveBeenCalledWith(
        expect.stringContaining("/v1/profile/theme"),
        expect.objectContaining({
          method: "POST",
          headers: expect.objectContaining({
            Authorization: "Bearer mock_token_123",
          }),
        })
      );
    });

    it("handles backend network failure gracefully without throwing", async () => {
      global.fetch = vi.fn().mockRejectedValue(new Error("Network offline"));

      const result = await syncThemeWithBackend(
        { boardTheme: "obsidian" },
        "mock_token_123"
      );
      expect(result).toBe(false);
    });

    it("fetches theme preferences from backend when token is available", async () => {
      global.fetch = vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({
          boardTheme: "neon",
          customPalette: {
            light: "#00f0ff",
            dark: "#7400b8",
            selected: "#ff007f",
            lastMove: "#39ff14",
          },
        }),
      } as Response);

      const prefs = await fetchThemePreferencesFromBackend("mock_token_123");
      expect(prefs).not.toBeNull();
      expect(prefs?.boardTheme).toBe("neon");
      expect(prefs?.customPalette?.light).toBe("#00f0ff");
    });
  });
});
