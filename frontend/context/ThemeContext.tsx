"use client";

import React, { createContext, useContext, useState, useEffect } from 'react';

export type BoardTheme = 'default' | 'cyberpunk' | 'classic_wood';

interface ThemeColors {
  light: string;
  dark: string;
}

export const THEME_COLORS: Record<BoardTheme, ThemeColors> = {
  default: { light: '#ffffff', dark: '#008e90' },
  cyberpunk: { light: '#ff007f', dark: '#7400b8' },
  classic_wood: { light: '#f0d9b5', dark: '#b58863' },
};

interface ThemeContextProps {
  boardTheme: BoardTheme;
  setBoardTheme: (theme: BoardTheme) => void;
  colors: ThemeColors;
}

const ThemeContext = createContext<ThemeContextProps | undefined>(undefined);

export const BoardThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [boardTheme, setBoardThemeState] = useState<BoardTheme>('default');

  useEffect(() => {
    const saved = localStorage.getItem('knightverse_board_theme') as BoardTheme;
    if (saved && THEME_COLORS[saved]) {
      setBoardThemeState(saved);
    }
  }, []);

  const setBoardTheme = (theme: BoardTheme) => {
    setBoardThemeState(theme);
    localStorage.setItem('knightverse_board_theme', theme);
  };

  return (
    <ThemeContext.Provider value={{ boardTheme, setBoardTheme, colors: THEME_COLORS[boardTheme] }}>
      {children}
    </ThemeContext.Provider>
  );
};

export const useBoardTheme = () => {
  const context = useContext(ThemeContext);
  if (!context) {
    throw new Error('useBoardTheme must be used within a ThemeProvider');
  }
  return context;
};
