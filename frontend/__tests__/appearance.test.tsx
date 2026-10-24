import React from "react";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render as rtlRender, screen, fireEvent } from "@testing-library/react";
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
import AppearanceSettings from "@/components/AppearanceSettings";
import { GamePreferencesProvider } from "@/context/GamePreferencesContext";
import { AuthProvider } from "@/context/authContext";

// BoardThemeProvider now calls useAuth() and GamePreferencesProvider wraps
// board components; both are required in every render in this suite.
const render = (ui: React.ReactElement, options?: Parameters<typeof rtlRender>[1]) =>
  rtlRender(ui, {
    wrapper: ({ children }) => (
      <AuthProvider>
        <GamePreferencesProvider>{children}</GamePreferencesProvider>
      </AuthProvider>
    ),
    ...options,
  });

describe("Theme Studio & Appearance Unit Tests", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    localStorage.clear();
    vi.unstubAllGlobals();
  });

  // ── 1. Preset Themes ─────────────────────────────────────────────────────

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

  // ── 2. Contrast Ratio ────────────────────────────────────────────────────

  describe("2. Contrast Ratio & Accessibility Calculations", () => {
    it("converts hex strings to RGB correctly", () => {
      expect(hexToRgb("#ffffff")).toEqual({ r: 255, g: 255, b: 255 });
      expect(hexToRgb("#000000")).toEqual({ r: 0, g: 0, b: 0 });
      expect(hexToRgb("fff")).toEqual({ r: 255, g: 255, b: 255 });
      expect(hexToRgb("invalid")).toBeNull();
    });

    it("calculates 21:1 contrast ratio between pure white and pure black", () => {
      expect(calculateContrastRatio("#ffffff", "#000000")).toBe(21);
    });

    it("calculates 1:1 contrast ratio for identical colors", () => {
      expect(calculateContrastRatio("#779954", "#779954")).toBe(1);
    });

    it("evaluates high contrast ratio as accessible with AAA rating", () => {
      const result = getContrastEvaluation(7.5);
      expect(result.isAccessible).toBe(true);
      expect(result.rating).toBe("AAA");
      expect(result.warning).toBeNull();
    });

    it("evaluates low contrast ratio with a warning banner", () => {
      const result = getContrastEvaluation(1.6);
      expect(result.isAccessible).toBe(false);
      expect(result.rating).toBe("POOR");
      expect(result.warning).toContain("Low contrast warning");
    });

    it("evaluates identical colors as FAIL with appropriate warning", () => {
      const result = getContrastEvaluation(1.0);
      expect(result.isAccessible).toBe(false);
      expect(result.rating).toBe("FAIL");
      expect(result.warning).toContain("Square colors are too similar or identical");
    });
  });

  // ── 3. ThemeContext State & Persistence ───────────────────────────────────

  describe("3. ThemeContext State & Persistence", () => {
    const TestConsumer = () => {
      const { boardTheme, setBoardTheme, colors, setCustomPalette, resetTheme, contrastRatio } =
        useBoardTheme();
      return (
        <div>
          <span data-testid="theme-id">{boardTheme}</span>
          <span data-testid="color-light">{colors.light}</span>
          <span data-testid="color-dark">{colors.dark}</span>
          <span data-testid="contrast">{contrastRatio}</span>
          <button onClick={() => setBoardTheme("wood")}>Set Wood</button>
          <button onClick={() => setBoardTheme("obsidian")}>Set Obsidian</button>
          <button onClick={() => setCustomPalette({ light: "#ffffff", dark: "#123456" })}>
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
      expect(screen.getByTestId("color-light").textContent).toBe(PRESET_THEMES.emerald.colors.light);
      expect(screen.getByTestId("color-dark").textContent).toBe(PRESET_THEMES.emerald.colors.dark);
    });

    it("switches presets and persists to localStorage", () => {
      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );
      fireEvent.click(screen.getByText("Set Wood"));
      expect(screen.getByTestId("theme-id").textContent).toBe("wood");
      expect(screen.getByTestId("color-light").textContent).toBe(PRESET_THEMES.wood.colors.light);
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
      expect(localStorage.getItem("knightverse_custom_board_palette")).toContain("#123456");
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
      expect(screen.getByTestId("color-dark").textContent).toBe(PRESET_THEMES.bordeaux.colors.dark);
    });

    it("updates CSS variables on document.documentElement", () => {
      render(
        <BoardThemeProvider>
          <TestConsumer />
        </BoardThemeProvider>
      );
      fireEvent.click(screen.getByText("Set Obsidian"));
      expect(document.documentElement.style.getPropertyValue("--board-dark")).toBe(
        PRESET_THEMES.obsidian.colors.dark
      );
      expect(document.documentElement.style.getPropertyValue("--board-light")).toBe(
        PRESET_THEMES.obsidian.colors.light
      );
    });
  });

  // ── 4. AppearanceSettings UI ──────────────────────────────────────────────

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
      expect(screen.getByLabelText(/selected square highlight color/i)).toBeInTheDocument();
      expect(screen.getByLabelText(/last move highlight color/i)).toBeInTheDocument();
      expect(screen.getByText(/board contrast ratio/i)).toBeInTheDocument();
    });

    it("allows modifying custom colors and displays live contrast ratio", () => {
      render(
        <BoardThemeProvider>
          <AppearanceSettings />
        </BoardThemeProvider>
      );
      fireEvent.click(screen.getByRole("button", { name: /custom palette/i }));
      fireEvent.change(screen.getByLabelText(/light square color/i), {
        target: { value: "#ffffff" },
      });
      fireEvent.change(screen.getByLabelText(/dark square color/i), {
        target: { value: "#000000" },
      });
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
      fireEvent.change(screen.getByLabelText(/light square color/i), {
        target: { value: "#555555" },
      });
      fireEvent.change(screen.getByLabelText(/dark square color/i), {
        target: { value: "#555555" },
      });
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
    });
  });

  // ── 5. Player Preference Sync ─────────────────────────────────────────────

  describe("5. Player Preference Sync", () => {
    it("keeps preferences local when no authentication token is available", async () => {
      const onWarning = vi.fn();
      const noTokenDeps = {
        getToken: () => null as string | null,
        refresh: async () => {},
        logout: async () => {},
      };

      const result = await syncThemeWithBackend(
        { boardTheme: "emerald" },
        noTokenDeps,
        onWarning
      );

      expect(result).toBe(false);
      expect(onWarning).not.toHaveBeenCalled();
    });

    it("sends authenticated preferences to the backend and loads them back", async () => {
      const fetchMock = vi.fn()
        .mockResolvedValueOnce({ ok: true, status: 200 } as Response)
        .mockResolvedValueOnce({
          ok: true,
          status: 200,
          json: async () => ({
            data: { preferences: { appearance: { boardTheme: "wood" } } },
          }),
        } as Response);
      vi.stubGlobal("fetch", fetchMock);

      const deps = {
        getToken: () => "mock_token_123" as string | null,
        refresh: async () => {},
        logout: async () => {},
      };

      const result = await syncThemeWithBackend({ boardTheme: "wood" }, deps);
      expect(result).toBe(true);
      expect(fetchMock).toHaveBeenCalledTimes(1);

      // headers is a Headers instance — check via .get()
      const headers: Headers = fetchMock.mock.calls[0][1].headers;
      expect(headers.get("Authorization")).toBe("Bearer mock_token_123");
      expect(headers.get("Content-Type")).toBe("application/json");
      expect(fetchMock.mock.calls[0][1].body).toBe(
        JSON.stringify({ appearance: { boardTheme: "wood" } })
      );

      const prefs = await fetchThemePreferencesFromBackend(deps);
      expect(prefs?.boardTheme).toBe("wood");
      const headers2: Headers = fetchMock.mock.calls[1][1].headers;
      expect(headers2.get("Authorization")).toBe("Bearer mock_token_123");
    });

    it("round-trips a custom palette through local storage", async () => {
      const customPalette = {
        light: "#00f0ff",
        dark: "#7400b8",
        selected: "#ff007f",
        lastMove: "#39ff14",
      };
      const noTokenDeps = {
        getToken: () => null as string | null,
        refresh: async () => {},
        logout: async () => {},
      };

      await syncThemeWithBackend({ boardTheme: "custom", customPalette }, noTokenDeps);

      const prefs = await fetchThemePreferencesFromBackend(noTokenDeps);
      expect(prefs).not.toBeNull();
      expect(prefs?.boardTheme).toBe("custom");
      expect(prefs?.customPalette?.light).toBe("#00f0ff");
    });

    it("returns null when no preferences have been saved", async () => {
      vi.stubGlobal(
        "fetch",
        vi.fn().mockResolvedValue({
          ok: true,
          status: 200,
          json: async () => ({ data: { preferences: {} } }),
        } as Response)
      );
      const deps = {
        getToken: () => "mock_token_123" as string | null,
        refresh: async () => {},
        logout: async () => {},
      };
      const prefs = await fetchThemePreferencesFromBackend(deps);
      expect(prefs).toBeNull();
    });
  });
});
