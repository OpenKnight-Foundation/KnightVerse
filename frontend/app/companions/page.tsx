'use client';

import React, { useState, useMemo } from 'react';
import { CompanionCard3D, AICompanion } from './CompanionCard3D';

const MOCK_COMPANIONS: AICompanion[] = [
  { id: '101', name: 'Stockfish Titan v16', level: 42, exp: 85, tacticalStyle: 'Aggressive', winRate: 91, winStreak: 12, image: '/images/ai-1.png', priceXLM: 150 },
  { id: '102', name: 'Leela Zero Sentinel', level: 38, exp: 64, tacticalStyle: 'Defensive', winRate: 88, winStreak: 8, image: '/images/ai-2.png', priceXLM: 120 },
  { id: '103', name: 'Kasparov Gambit AI', level: 50, exp: 95, tacticalStyle: 'Tactical', winRate: 94, winStreak: 19, image: '/images/ai-3.png', priceXLM: 250 },
  { id: '104', name: 'Capablanca Endgames', level: 35, exp: 40, tacticalStyle: 'Endgame Master', winRate: 86, winStreak: 5, image: '/images/ai-4.png', priceXLM: 90 },
];

const STYLES = ['All', 'Aggressive', 'Defensive', 'Tactical', 'Endgame Master'] as const;

export default function AICompanionsPage() {
  const [selectedStyle, setSelectedStyle] = useState<string>('All');
  const [searchQuery, setSearchQuery] = useState('');

  const filteredCompanions = useMemo(() => {
    return MOCK_COMPANIONS.filter((c) => {
      const matchesStyle = selectedStyle === 'All' || c.tacticalStyle === selectedStyle;
      const matchesSearch = c.name.toLowerCase().includes(searchQuery.toLowerCase());
      return matchesStyle && matchesSearch;
    });
  }, [selectedStyle, searchQuery]);

  const handleMint = (id: string) => {
    alert(`Initiating Soroban contract call to mint AI Companion #${id}...`);
  };

  const handleRent = (id: string) => {
    alert(`Connecting to Soroban rental market for Companion #${id}...`);
  };

  return (
    <main className="min-h-screen bg-slate-950 text-slate-100 p-8 md:p-12">
      <div className="max-w-7xl mx-auto">
        <div className="flex flex-col md:flex-row md:items-center md:justify-between mb-8 gap-4">
          <div>
            <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight bg-gradient-to-r from-indigo-400 to-cyan-400 bg-clip-text text-transparent">
              AI Companion NFT Gallery
            </h1>
            <p className="text-slate-400 text-sm mt-1">
              Recruit, rent, and own decentralized chess AI agents powered by Soroban & PyTorch.
            </p>
          </div>
          <input
            type="text"
            placeholder="Search agents by name..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="px-4 py-2 bg-slate-900 border border-slate-800 rounded-xl focus:outline-none focus:border-indigo-500 text-sm w-full md:w-72"
          />
        </div>

        <div className="flex flex-wrap gap-2 mb-8">
          {STYLES.map((style) => (
            <button
              key={style}
              onClick={() => setSelectedStyle(style)}
              className={`px-4 py-2 rounded-xl text-xs font-semibold transition-all ${
                selectedStyle === style
                  ? 'bg-indigo-600 text-white shadow-lg shadow-indigo-600/30'
                  : 'bg-slate-900 text-slate-400 hover:bg-slate-800 border border-slate-800'
              }`}
            >
              {style}
            </button>
          ))}
        </div>

        {filteredCompanions.length > 0 ? (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-6">
            {filteredCompanions.map((companion) => (
              <CompanionCard3D
                key={companion.id}
                companion={companion}
                onMint={handleMint}
                onRent={handleRent}
              />
            ))}
          </div>
        ) : (
          <div className="text-center py-20 text-slate-500">
            No AI companions found matching your criteria.
          </div>
        )}
      </div>
    </main>
  );
}
