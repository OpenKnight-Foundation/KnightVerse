"use client";

import React from 'react';
import { useSound } from '@/context/SoundContext';

export default function SoundSettings() {
  const { volume, setVolume, isMuted, setIsMuted, soundPack, setSoundPack } = useSound();

  const soundPacks: { id: 'standard' | 'arcade' | 'minimalist'; label: string }[] = [
    { id: 'standard', label: 'Standard' },
    { id: 'arcade', label: 'Arcade' },
    { id: 'minimalist', label: 'Minimalist' },
  ];

  return (
    <div className="p-6 bg-gray-900 rounded-xl shadow-lg border border-gray-800 text-white max-w-md mx-auto mt-10">
      <h2 className="text-2xl font-bold mb-6 text-teal-400">Audio Settings</h2>
      
      <div className="mb-6">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-lg font-semibold text-gray-300">Master Volume</h3>
          <button
            onClick={() => setIsMuted(!isMuted)}
            className={`px-3 py-1 rounded-md text-sm font-medium transition-colors ${
              isMuted ? 'bg-red-500/20 text-red-400 border border-red-500/50' : 'bg-gray-800 text-gray-300 border border-gray-700 hover:bg-gray-700'
            }`}
          >
            {isMuted ? 'Unmute' : 'Mute'}
          </button>
        </div>
        
        <div className="flex items-center space-x-4">
          <span className="text-gray-400 text-sm">0%</span>
          <input
            type="range"
            min="0"
            max="100"
            value={volume}
            onChange={(e) => setVolume(Number(e.target.value))}
            disabled={isMuted}
            className={`flex-1 h-2 rounded-lg appearance-none cursor-pointer ${isMuted ? 'bg-gray-700 opacity-50' : 'bg-gradient-to-r from-teal-500 to-blue-600'}`}
          />
          <span className="text-gray-400 text-sm">100%</span>
        </div>
      </div>

      <div className="mb-6">
        <h3 className="text-lg font-semibold mb-3 text-gray-300">Sound Pack</h3>
        <div className="flex flex-col space-y-3">
          {soundPacks.map((pack) => (
            <button
              key={pack.id}
              onClick={() => setSoundPack(pack.id)}
              className={`px-4 py-3 text-left rounded-lg transition-all duration-300 flex items-center justify-between ${
                soundPack === pack.id
                  ? 'bg-gradient-to-r from-teal-600 to-blue-700 text-white font-semibold shadow-md'
                  : 'bg-gray-800 hover:bg-gray-700 text-gray-300 border border-gray-700'
              }`}
            >
              <span>{pack.label}</span>
              {soundPack === pack.id && (
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                </svg>
              )}
            </button>
          ))}
        </div>
      </div>
      
      <p className="text-sm text-gray-400">
        Assets for your selected sound pack are pre-loaded to prevent latency.
      </p>
    </div>
  );
}
