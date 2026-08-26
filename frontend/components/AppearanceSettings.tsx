"use client";

import React from 'react';
import { useBoardTheme, BoardTheme } from '@/context/ThemeContext';

export default function AppearanceSettings() {
  const { boardTheme, setBoardTheme } = useBoardTheme();

  const themes: { id: BoardTheme; label: string }[] = [
    { id: 'default', label: 'Default' },
    { id: 'cyberpunk', label: 'Cyberpunk' },
    { id: 'classic_wood', label: 'Classic Wood' },
  ];

  return (
    <div className="p-6 bg-gray-900 rounded-xl shadow-lg border border-gray-800 text-white max-w-md mx-auto mt-10">
      <h2 className="text-2xl font-bold mb-6 text-teal-400">Appearance Settings</h2>
      <div className="mb-6">
        <h3 className="text-lg font-semibold mb-3 text-gray-300">Board Theme</h3>
        <div className="flex flex-col space-y-3">
          {themes.map((theme) => (
            <button
              key={theme.id}
              onClick={() => setBoardTheme(theme.id)}
              className={`px-4 py-3 text-left rounded-lg transition-all duration-300 flex items-center justify-between ${
                boardTheme === theme.id
                  ? 'bg-gradient-to-r from-teal-600 to-blue-700 text-white font-semibold shadow-md'
                  : 'bg-gray-800 hover:bg-gray-700 text-gray-300 border border-gray-700'
              }`}
            >
              <span>{theme.label}</span>
              {boardTheme === theme.id && (
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              )}
            </button>
          ))}
        </div>
      </div>
      <p className="text-sm text-gray-400">
        Changes to your theme are saved automatically and will apply to all your games.
      </p>
    </div>
  );
}
