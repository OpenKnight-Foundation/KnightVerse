"use client";

import React, { useEffect, useRef } from "react";

interface MoveHistoryProps {
  history: string[]; // Array of SAN moves
  onMoveClick?: (index: number) => void;
  currentMoveIndex?: number;
}

export function MoveHistory({ history, onMoveClick, currentMoveIndex = history.length - 1 }: MoveHistoryProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  
  // Group moves into pairs (White, Black)
  const movePairs: string[][] = [];
  for (let i = 0; i < history.length; i += 2) {
    movePairs.push([history[i], history[i + 1] || ""]);
  }

  // Auto-scroll to the bottom when new moves are added
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight;
    }
  }, [history.length]);

  return (
    <div className="flex flex-col h-[320px] md:h-[560px] bg-gray-900 border border-gray-800 rounded-lg overflow-hidden shadow-lg w-full max-w-[280px]">
      <div className="bg-gray-800 p-3 border-b border-gray-700 shadow-sm z-10">
        <h3 className="text-white font-semibold text-sm flex items-center gap-2">
          <svg className="w-4 h-4 text-teal-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          Move History
        </h3>
      </div>
      <div ref={containerRef} className="flex-1 overflow-y-auto p-2 space-y-1 custom-scrollbar">
        {movePairs.map((pair, idx) => {
          const moveNumber = idx + 1;
          const whiteIndex = idx * 2;
          const blackIndex = idx * 2 + 1;

          return (
            <div key={idx} className="flex text-sm items-center hover:bg-gray-800/50 rounded transition-colors group">
              <div className="w-10 text-gray-500 text-right pr-3 py-1 font-mono text-xs">{moveNumber}.</div>
              <div className="flex-1 flex space-x-1">
                <button 
                  onClick={() => onMoveClick?.(whiteIndex)}
                  className={`flex-1 text-left px-2 py-1 rounded transition-all duration-200 ${
                    currentMoveIndex === whiteIndex 
                      ? "bg-teal-600/80 text-white font-bold shadow-sm" 
                      : "text-gray-300 hover:bg-gray-700/80"
                  }`}
                >
                  {pair[0]}
                </button>
                {pair[1] && (
                  <button 
                    onClick={() => onMoveClick?.(blackIndex)}
                    className={`flex-1 text-left px-2 py-1 rounded transition-all duration-200 ${
                      currentMoveIndex === blackIndex 
                        ? "bg-teal-600/80 text-white font-bold shadow-sm" 
                        : "text-gray-300 hover:bg-gray-700/80"
                    }`}
                  >
                    {pair[1]}
                  </button>
                )}
              </div>
            </div>
          );
        })}
        {history.length === 0 && (
          <div className="text-gray-500 text-center py-8 text-sm italic flex flex-col items-center gap-2">
            <svg className="w-8 h-8 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 12h.01M12 12h.01M16 12h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            No moves played yet
          </div>
        )}
      </div>
    </div>
  );
}
