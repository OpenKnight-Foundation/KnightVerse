import { render, screen, fireEvent } from "@testing-library/react";
import { GamePreferencesProvider, useGamePreferences } from "@/context/GamePreferencesContext";
import AppearanceSettings from "@/components/AppearanceSettings";
import { BoardThemeProvider } from "@/context/ThemeContext";

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: (key: string) => store[key] || null,
    setItem: (key: string, value: string) => {
      store[key] = value.toString();
    },
    clear: () => {
      store = {};
    },
  };
})();
Object.defineProperty(window, "localStorage", { value: localStorageMock });

describe("Piece Set Functionality", () => {
  beforeEach(() => {
    localStorageMock.clear();
  });

  it("loads default piece set as neo", () => {
    render(
      <BoardThemeProvider>
        <GamePreferencesProvider>
          <AppearanceSettings />
        </GamePreferencesProvider>
      </BoardThemeProvider>
    );
    
    const selector = screen.getByTestId("piece-set-selector");
    expect(selector).toHaveValue("neo");
  });

  it("persists piece set selection to localStorage", () => {
    render(
      <BoardThemeProvider>
        <GamePreferencesProvider>
          <AppearanceSettings />
        </GamePreferencesProvider>
      </BoardThemeProvider>
    );
    
    const selector = screen.getByTestId("piece-set-selector");
    fireEvent.change(selector, { target: { value: "cyberpunk" } });
    
    const savedPreferences = JSON.parse(localStorage.getItem("knightverse_game_preferences") || "{}");
    expect(savedPreferences.pieceSet).toBe("cyberpunk");
  });

  it("loads saved piece set from localStorage", () => {
    localStorage.setItem("knightverse_game_preferences", JSON.stringify({ pieceSet: "medieval" }));
    
    render(
      <BoardThemeProvider>
        <GamePreferencesProvider>
          <AppearanceSettings />
        </GamePreferencesProvider>
      </BoardThemeProvider>
    );
    
    const selector = screen.getByTestId("piece-set-selector");
    expect(selector).toHaveValue("medieval");
  });

  it("allows switching between all piece sets", () => {
    render(
      <BoardThemeProvider>
        <GamePreferencesProvider>
          <AppearanceSettings />
        </GamePreferencesProvider>
      </BoardThemeProvider>
    );
    
    const selector = screen.getByTestId("piece-set-selector");
    const pieceSets = ["neo", "staunton", "alpha", "medieval", "cyberpunk"];
    
    pieceSets.forEach((set) => {
      fireEvent.change(selector, { target: { value: set } });
      expect(selector).toHaveValue(set);
    });
  });
});