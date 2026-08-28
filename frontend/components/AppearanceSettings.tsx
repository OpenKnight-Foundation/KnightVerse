"use client";

import React, { useState, useMemo, useCallback } from "react";
import {
  useBoardTheme,
  PRESET_THEMES,
  PresetTheme,
  ThemeColors,
  calculateContrastRatio,
  getContrastEvaluation,
} from "@/context/ThemeContext";
import { useGamePreferences, PieceSet } from "@/context/GamePreferencesContext";
import Image from "next/image";
import {
  Palette,
  Check,
  RotateCcw,
  Sliders,
  AlertTriangle,
  CheckCircle2,
  Cloud,
  Sparkles,
  Info,
  ChevronDown,
} from "lucide-react";

// Import preview piece images
import NeoWhiteKing from "./chess/chesspieces/neo/white-king.svg";
import StauntonWhiteKing from "./chess/chesspieces/staunton/white-king.svg";
import AlphaWhiteKing from "./chess/chesspieces/alpha/white-king.svg";
import MedievalWhiteKing from "./chess/chesspieces/medieval/white-king.svg";
import CyberpunkWhiteKing from "./chess/chesspieces/cyberpunk/white-king.svg";

const PIECE_SET_PREVIEWS: Record<PieceSet, string> = {
  neo: NeoWhiteKing,
  staunton: StauntonWhiteKing,
  alpha: AlphaWhiteKing,
  medieval: MedievalWhiteKing,
  cyberpunk: CyberpunkWhiteKing,
};

const PIECE_SETS: { id: PieceSet; label: string; description: string }[] = [
  { id: "neo", label: "Standard Neo", description: "Modern minimalist vector pieces" },
  { id: "staunton", label: "Staunton Classic", description: "Traditional tournament-style pieces" },
  { id: "alpha", label: "Alpha", description: "Sleek geometric abstract design" },
  { id: "medieval", label: "Medieval Knight", description: "Ornate historical fantasy pieces" },
  { id: "cyberpunk", label: "Cyberpunk Glow", description: "Neon-lit futuristic aesthetic" },
];

// Sample pieces for interactive preview board (8x8 mini standard layout or demo layout)
const PREVIEW_BOARD_SETUP: string[][] = [
  ["bR", "bN", "bB", "bQ", "bK", "bB", "bN", "bR"],
  ["bP", "bP", "bP", "bP", "bP", "bP", "bP", "bP"],
  ["", "", "", "", "", "", "", ""],
  ["", "", "", "", "", "", "", ""],
  ["", "", "", "", "", "", "", ""],
  ["", "", "", "", "", "", "", ""],
  ["wP", "wP", "wP", "wP", "wP", "wP", "wP", "wP"],
  ["wR", "wN", "wB", "wQ", "wK", "wB", "wN", "wR"],
];

const PIECE_UNICODE: Record<string, string> = {
  wK: "♔",
  wQ: "♕",
  wR: "♖",
  wB: "♗",
  wN: "♘",
  wP: "♙",
  bK: "♚",
  bQ: "♛",
  bR: "♜",
  bB: "♝",
  bN: "♞",
  bP: "♟",
};

export default function AppearanceSettings() {
  const {
    boardTheme,
    setBoardTheme,
    customPalette,
    setCustomPalette,
    colors,
    resetTheme,
    isSynced,
  } = useBoardTheme();
  const { preferences, setPreference } = useGamePreferences();

  const [activeTab, setActiveTab] = useState<"presets" | "custom">("presets");
  const [previewSelectedSquare, setPreviewSelectedSquare] = useState<string>("6,4"); // e2
  const [previewLastMove, setPreviewLastMove] = useState<{ from: string; to: string }>({
    from: "1,4", // e7
    to: "3,4", // e5
  });

  // Local draft state for custom hex inputs
  const [customDraft, setCustomDraft] = useState<ThemeColors>(customPalette);

  // Sync draft when customPalette changes from outside
  React.useEffect(() => {
    setCustomDraft(customPalette);
  }, [customPalette]);

  // Evaluate contrast for current draft
  const draftContrastRatio = useMemo(() => {
    return calculateContrastRatio(customDraft.light, customDraft.dark);
  }, [customDraft.light, customDraft.dark]);

  const draftContrastEval = useMemo(() => {
    return getContrastEvaluation(draftContrastRatio);
  }, [draftContrastRatio]);

  // Preset list array
  const presetList = useMemo(() => Object.values(PRESET_THEMES), []);

  const handlePresetSelect = useCallback(
    (presetId: PresetTheme) => {
      setBoardTheme(presetId);
    },
    [setBoardTheme]
  );

  const handleCustomColorChange = useCallback(
    (key: keyof ThemeColors, value: string) => {
      // Clean hex value
      const cleanValue = value.startsWith("#") ? value : `#${value}`;
      const newDraft = { ...customDraft, [key]: cleanValue };
      setCustomDraft(newDraft);

      // Validate hex length before saving
      if (/^#[0-9A-Fa-f]{6}$/.test(cleanValue)) {
        // Enforce minimum contrast to avoid identical colors
        const newRatio = calculateContrastRatio(
          key === "light" ? cleanValue : customDraft.light,
          key === "dark" ? cleanValue : customDraft.dark
        );

        if (newRatio > 1.1) {
          setCustomPalette({ [key]: cleanValue });
        }
      }
    },
    [customDraft, setCustomPalette]
  );

  const handlePreviewSquareClick = (r: number, c: number) => {
    const key = `${r},${c}`;
    if (previewSelectedSquare === key) {
      setPreviewSelectedSquare("");
    } else {
      setPreviewSelectedSquare(key);
      setPreviewLastMove((prev) => ({
        from: prev.to,
        to: key,
      }));
    }
  };

  return (
    <div
      className="bg-gray-900 rounded-2xl shadow-2xl border border-gray-800 text-white p-6 md:p-8 max-w-4xl mx-auto w-full transition-all duration-300"
      data-testid="appearance-settings"
    >
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-6 border-b border-gray-800">
        <div>
          <div className="flex items-center space-x-3">
            <div className="p-2.5 rounded-xl bg-teal-500/10 text-teal-400 border border-teal-500/20">
              <Palette className="w-6 h-6" />
            </div>
            <div>
              <h2 className="text-2xl font-bold bg-gradient-to-r from-teal-400 to-blue-400 bg-clip-text text-transparent">
                Chessboard Theme Studio
              </h2>
              <p className="text-sm text-gray-400 mt-0.5">
                Customize your chessboard aesthetics with live real-time previews.
              </p>
            </div>
          </div>
        </div>

        <div className="flex items-center space-x-3">
          {/* Sync status */}
          <div
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-gray-800/80 border border-gray-700 text-xs font-medium text-gray-300"
            title={isSynced ? "Theme synced with your cloud profile" : "Theme saved locally"}
          >
            {isSynced ? (
              <>
                <Cloud className="w-3.5 h-3.5 text-teal-400 animate-pulse" />
                <span className="text-teal-300">Cloud Synced</span>
              </>
            ) : (
              <>
                <CheckCircle2 className="w-3.5 h-3.5 text-gray-400" />
                <span>Saved Locally</span>
              </>
            )}
          </div>

          {/* Reset button */}
          <button
            type="button"
            onClick={resetTheme}
            className="flex items-center space-x-1.5 px-3 py-1.5 rounded-lg bg-gray-800 hover:bg-gray-700 text-gray-300 hover:text-white border border-gray-700 text-xs font-medium transition-all duration-200"
            aria-label="Reset theme to defaults"
          >
            <RotateCcw className="w-3.5 h-3.5" />
            <span>Reset</span>
          </button>
        </div>
      </div>

      {/* Main Studio Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 mt-6">
        {/* Left Column: Palette Controls */}
        <div className="lg:col-span-7 flex flex-col space-y-6">
          {/* Piece Set Selector */}
          <div className="space-y-3 bg-gray-800/40 border border-gray-800 p-5 rounded-xl">
            <div className="flex items-center justify-between">
              <label className="text-sm font-semibold text-white">Chess Piece Set</label>
              <div className="flex items-center space-x-2">
                <div className="w-10 h-10 relative">
                  <Image
                    src={PIECE_SET_PREVIEWS[preferences.pieceSet]}
                    alt="Current piece set preview"
                    fill
                    className="object-contain"
                    sizes="40px"
                  />
                </div>
                <div className="relative">
                  <select
                    value={preferences.pieceSet}
                    onChange={(e) => setPreference("pieceSet", e.target.value as PieceSet)}
                    className="appearance-none bg-gray-900 border border-gray-700 rounded-lg px-4 py-2 pr-10 text-sm text-white cursor-pointer hover:border-gray-600 focus:outline-none focus:ring-2 focus:ring-teal-500/50 transition-all duration-200"
                    data-testid="piece-set-selector"
                  >
                    {PIECE_SETS.map((set) => (
                      <option key={set.id} value={set.id}>
                        {set.label}
                      </option>
                    ))}
                  </select>
                  <ChevronDown className="absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400 pointer-events-none" />
                </div>
              </div>
            </div>
            <p className="text-xs text-gray-400">
              {PIECE_SETS.find(set => set.id === preferences.pieceSet)?.description}
            </p>
            
            {/* Piece set grid preview */}
            <div className="grid grid-cols-5 gap-3 mt-4">
              {PIECE_SETS.map((set) => (
                <button
                  key={set.id}
                  type="button"
                  onClick={() => setPreference("pieceSet", set.id)}
                  className={`p-3 rounded-lg border transition-all duration-200 flex flex-col items-center space-y-2 ${
                    preferences.pieceSet === set.id
                      ? "bg-gray-900 border-teal-500 ring-2 ring-teal-500/30"
                      : "bg-gray-900/50 border-gray-700 hover:border-gray-600"
                  }`}
                >
                  <div className="w-8 h-8 relative">
                    <Image
                      src={PIECE_SET_PREVIEWS[set.id]}
                      alt={set.label}
                      fill
                      className="object-contain"
                      sizes="32px"
                    />
                  </div>
                  <span className="text-[10px] text-gray-400 text-center leading-tight">{set.label}</span>
                  {preferences.pieceSet === set.id && (
                    <Check className="w-3 h-3 text-teal-400" />
                  )}
                </button>
              ))}
            </div>
          </div>

          {/* Studio Mode Tabs */}
          <div className="flex p-1 bg-gray-800/80 backdrop-blur-sm rounded-xl border border-gray-700/60">
            <button
              type="button"
              onClick={() => {
                setActiveTab("presets");
                if (boardTheme === "custom") {
                  setBoardTheme("emerald");
                }
              }}
              className={`flex-1 flex items-center justify-center space-x-2 py-2.5 px-4 rounded-lg text-sm font-semibold transition-all duration-200 ${
                activeTab === "presets"
                  ? "bg-gradient-to-r from-teal-600 to-blue-600 text-white shadow-md shadow-teal-500/20"
                  : "text-gray-400 hover:text-gray-200"
              }`}
            >
              <Sparkles className="w-4 h-4" />
              <span>Curated Presets</span>
            </button>
            <button
              type="button"
              onClick={() => {
                setActiveTab("custom");
                setBoardTheme("custom");
              }}
              className={`flex-1 flex items-center justify-center space-x-2 py-2.5 px-4 rounded-lg text-sm font-semibold transition-all duration-200 ${
                activeTab === "custom"
                  ? "bg-gradient-to-r from-teal-600 to-blue-600 text-white shadow-md shadow-teal-500/20"
                  : "text-gray-400 hover:text-gray-200"
              }`}
            >
              <Sliders className="w-4 h-4" />
              <span>Custom Palette</span>
            </button>
          </div>

          {/* Curated Presets Grid */}
          {activeTab === "presets" && (
            <div className="space-y-3" role="radiogroup" aria-label="Curated Board Theme Presets">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                {presetList.map((preset) => {
                  const isSelected = boardTheme === preset.id;
                  return (
                    <button
                      key={preset.id}
                      type="button"
                      role="radio"
                      aria-checked={isSelected}
                      onClick={() => handlePresetSelect(preset.id)}
                      className={`p-4 rounded-xl text-left border transition-all duration-200 flex flex-col justify-between space-y-3 relative group overflow-hidden ${
                        isSelected
                          ? "bg-gray-800/90 border-teal-500 ring-2 ring-teal-500/30 shadow-lg shadow-teal-500/10"
                          : "bg-gray-800/40 border-gray-800 hover:bg-gray-800/80 hover:border-gray-700"
                      }`}
                    >
                      <div className="flex items-center justify-between w-full">
                        <div className="font-semibold text-white group-hover:text-teal-300 transition-colors">
                          {preset.label}
                        </div>
                        {isSelected && (
                          <div className="w-5 h-5 rounded-full bg-teal-500 flex items-center justify-center text-white">
                            <Check className="w-3.5 h-3.5" />
                          </div>
                        )}
                      </div>

                      <p className="text-xs text-gray-400 line-clamp-2 leading-relaxed">
                        {preset.description}
                      </p>

                      {/* Color swatch pill row */}
                      <div className="flex items-center space-x-1.5 pt-1">
                        <div
                          className="w-5 h-5 rounded-md border border-black/20 shadow-inner"
                          style={{ backgroundColor: preset.colors.light }}
                          title={`Light: ${preset.colors.light}`}
                        />
                        <div
                          className="w-5 h-5 rounded-md border border-black/20 shadow-inner"
                          style={{ backgroundColor: preset.colors.dark }}
                          title={`Dark: ${preset.colors.dark}`}
                        />
                        <div
                          className="w-5 h-5 rounded-md border border-black/20 shadow-inner"
                          style={{ backgroundColor: preset.colors.selected }}
                          title={`Selected: ${preset.colors.selected}`}
                        />
                        <div
                          className="w-5 h-5 rounded-md border border-black/20 shadow-inner"
                          style={{ backgroundColor: preset.colors.lastMove }}
                          title={`Last Move: ${preset.colors.lastMove}`}
                        />
                      </div>
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Custom Palette Builder */}
          {activeTab === "custom" && (
            <div className="space-y-5 bg-gray-800/40 border border-gray-800 p-5 rounded-xl">
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                {/* Light Square Picker */}
                <div className="space-y-2">
                  <label
                    htmlFor="light-color-input"
                    className="block text-xs font-semibold text-gray-300 uppercase tracking-wider"
                  >
                    Light Square
                  </label>
                  <div className="flex items-center space-x-2 bg-gray-900 border border-gray-700/80 rounded-lg p-1.5">
                    <input
                      id="light-color-input"
                      type="color"
                      value={customDraft.light}
                      onChange={(e) => handleCustomColorChange("light", e.target.value)}
                      className="w-8 h-8 rounded border-0 cursor-pointer bg-transparent"
                      aria-label="Light square color"
                    />
                    <input
                      type="text"
                      value={customDraft.light}
                      onChange={(e) => handleCustomColorChange("light", e.target.value)}
                      className="w-full bg-transparent text-sm font-mono text-gray-200 outline-none uppercase"
                      maxLength={7}
                      placeholder="#FFFFFF"
                    />
                  </div>
                </div>

                {/* Dark Square Picker */}
                <div className="space-y-2">
                  <label
                    htmlFor="dark-color-input"
                    className="block text-xs font-semibold text-gray-300 uppercase tracking-wider"
                  >
                    Dark Square
                  </label>
                  <div className="flex items-center space-x-2 bg-gray-900 border border-gray-700/80 rounded-lg p-1.5">
                    <input
                      id="dark-color-input"
                      type="color"
                      value={customDraft.dark}
                      onChange={(e) => handleCustomColorChange("dark", e.target.value)}
                      className="w-8 h-8 rounded border-0 cursor-pointer bg-transparent"
                      aria-label="Dark square color"
                    />
                    <input
                      type="text"
                      value={customDraft.dark}
                      onChange={(e) => handleCustomColorChange("dark", e.target.value)}
                      className="w-full bg-transparent text-sm font-mono text-gray-200 outline-none uppercase"
                      maxLength={7}
                      placeholder="#008E90"
                    />
                  </div>
                </div>

                {/* Selected Square Highlight */}
                <div className="space-y-2">
                  <label
                    htmlFor="selected-color-input"
                    className="block text-xs font-semibold text-gray-300 uppercase tracking-wider"
                  >
                    Selection Highlight
                  </label>
                  <div className="flex items-center space-x-2 bg-gray-900 border border-gray-700/80 rounded-lg p-1.5">
                    <input
                      id="selected-color-input"
                      type="color"
                      value={customDraft.selected}
                      onChange={(e) => handleCustomColorChange("selected", e.target.value)}
                      className="w-8 h-8 rounded border-0 cursor-pointer bg-transparent"
                      aria-label="Selected square highlight color"
                    />
                    <input
                      type="text"
                      value={customDraft.selected}
                      onChange={(e) => handleCustomColorChange("selected", e.target.value)}
                      className="w-full bg-transparent text-sm font-mono text-gray-200 outline-none uppercase"
                      maxLength={7}
                      placeholder="#00BCD4"
                    />
                  </div>
                </div>

                {/* Last Move Highlight */}
                <div className="space-y-2">
                  <label
                    htmlFor="lastmove-color-input"
                    className="block text-xs font-semibold text-gray-300 uppercase tracking-wider"
                  >
                    Last Move Highlight
                  </label>
                  <div className="flex items-center space-x-2 bg-gray-900 border border-gray-700/80 rounded-lg p-1.5">
                    <input
                      id="lastmove-color-input"
                      type="color"
                      value={customDraft.lastMove}
                      onChange={(e) => handleCustomColorChange("lastMove", e.target.value)}
                      className="w-8 h-8 rounded border-0 cursor-pointer bg-transparent"
                      aria-label="Last move highlight color"
                    />
                    <input
                      type="text"
                      value={customDraft.lastMove}
                      onChange={(e) => handleCustomColorChange("lastMove", e.target.value)}
                      className="w-full bg-transparent text-sm font-mono text-gray-200 outline-none uppercase"
                      maxLength={7}
                      placeholder="#80DEEA"
                    />
                  </div>
                </div>
              </div>

              {/* Contrast Ratio & Accessibility Evaluation */}
              <div className="p-4 rounded-xl bg-gray-900/90 border border-gray-700/80 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
                    Board Contrast Ratio
                  </span>
                  <div className="flex items-center space-x-2">
                    <span className="text-sm font-mono font-bold text-white">
                      {draftContrastRatio}:1
                    </span>
                    <span
                      className={`text-xs px-2 py-0.5 rounded font-semibold ${
                        draftContrastEval.rating === "AAA"
                          ? "bg-green-500/20 text-green-400 border border-green-500/30"
                          : draftContrastEval.rating === "AA"
                          ? "bg-teal-500/20 text-teal-400 border border-teal-500/30"
                          : draftContrastEval.rating === "POOR"
                          ? "bg-yellow-500/20 text-yellow-400 border border-yellow-500/30"
                          : "bg-red-500/20 text-red-400 border border-red-500/30"
                      }`}
                    >
                      {draftContrastEval.rating === "AAA"
                        ? "WCAG AAA"
                        : draftContrastEval.rating === "AA"
                        ? "WCAG AA"
                        : draftContrastEval.rating === "POOR"
                        ? "Low Contrast"
                        : "Fail (Identical)"}
                    </span>
                  </div>
                </div>

                {/* Contrast warning banner */}
                {draftContrastEval.warning && (
                  <div
                    className="flex items-start space-x-2 text-xs text-amber-300 bg-amber-500/10 border border-amber-500/30 p-2.5 rounded-lg"
                    role="alert"
                  >
                    <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0 mt-0.5" />
                    <span>{draftContrastEval.warning}</span>
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Right Column: Live Interactive Preview Board */}
        <div className="lg:col-span-5 flex flex-col items-center justify-center bg-gray-950/60 p-5 rounded-2xl border border-gray-800/80">
          <div className="flex items-center justify-between w-full mb-3 px-1">
            <div className="flex items-center space-x-1.5 text-xs font-semibold text-gray-300 uppercase tracking-wider">
              <Sparkles className="w-3.5 h-3.5 text-teal-400" />
              <span>Live Interactive Preview</span>
            </div>
            <span className="text-[11px] text-gray-500">Click squares to test</span>
          </div>

          {/* Interactive Chessboard Preview */}
          <div
            className="w-full max-w-[320px] aspect-square rounded-lg border-2 border-gray-700 shadow-2xl overflow-hidden grid grid-cols-8 grid-rows-8"
            style={{
              boxShadow: `0 10px 25px -5px ${colors.dark}40`,
            }}
            role="grid"
            aria-label="Live theme preview board"
          >
            {PREVIEW_BOARD_SETUP.map((row, r) =>
              row.map((piece, c) => {
                const squareKey = `${r},${c}`;
                const isLight = (r + c) % 2 === 0;
                const isSelected = previewSelectedSquare === squareKey;
                const isLastMove =
                  previewLastMove.from === squareKey || previewLastMove.to === squareKey;

                return (
                  <button
                    key={squareKey}
                    type="button"
                    role="gridcell"
                    onClick={() => handlePreviewSquareClick(r, c)}
                    aria-label={`Square ${String.fromCharCode(97 + c)}${8 - r}`}
                    className="w-full h-full flex items-center justify-center text-lg md:text-xl font-bold select-none cursor-pointer transition-all duration-150 relative"
                    style={{
                      backgroundColor: isLight ? colors.light : colors.dark,
                      boxShadow: isSelected
                        ? `inset 0 0 0 3px ${colors.selected}`
                        : isLastMove
                        ? `inset 0 0 0 2px ${colors.lastMove}`
                        : "none",
                      color: piece.startsWith("w") ? "#f8fafc" : "#0f172a",
                      textShadow: piece.startsWith("w")
                        ? "0 1px 2px rgba(0,0,0,0.8)"
                        : "0 1px 2px rgba(255,255,255,0.4)",
                    }}
                  >
                    {piece ? PIECE_UNICODE[piece] : ""}
                  </button>
                );
              })
            )}
          </div>

          <div className="mt-4 flex items-center space-x-2 text-xs text-gray-400 text-center">
            <Info className="w-3.5 h-3.5 text-gray-500 shrink-0" />
            <span>Updates live across all game boards instantly without page refresh.</span>
          </div>
        </div>
      </div>
    </div>
  );
}