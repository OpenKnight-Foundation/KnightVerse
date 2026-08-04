import React, { useState } from 'react';

interface GameMove {
  moveNumber: number;
  move: string;
  evaluation: number;
  bestMove?: string;
  isMistake?: boolean;
  isBlunder?: boolean;
}

export const AnalysisBoard: React.FC<{ moves: GameMove[] }> = ({ moves }) => {
  const [currentMoveIndex, setCurrentMoveIndex] = useState(0);
  const [arrows, setArrows] = useState<Array<{ from: string; to: string }>>([]);

  const currentMove = moves[currentMoveIndex];

  const handlePrevMove = () => setCurrentMoveIndex(Math.max(0, currentMoveIndex - 1));
  const handleNextMove = () => setCurrentMoveIndex(Math.min(moves.length - 1, currentMoveIndex + 1));

  return (
    <div className="flex gap-8 p-8 bg-gray-900">
      <div className="flex-1">
        <div className="w-96 h-96 bg-amber-100 border-4 border-amber-900 relative" />
        <div className="flex gap-4 mt-4">
          <button onClick={handlePrevMove} className="px-4 py-2 bg-blue-600 text-white rounded">← Previous</button>
          <button onClick={handleNextMove} className="px-4 py-2 bg-blue-600 text-white rounded">Next →</button>
        </div>
      </div>
      <div className="flex-1">
        <h2 className="text-2xl font-bold text-white mb-4">Move Analysis</h2>
        <div className="space-y-2 text-white">
          {moves.map((m, i) => (
            <div key={i} className={`p-2 rounded ${m.isBlunder ? 'bg-red-600' : m.isMistake ? 'bg-orange-600' : 'bg-gray-700'}`}>
              <span className="font-bold">{m.moveNumber}. {m.move}</span>
              {m.bestMove && <span className="ml-2 text-sm">Best: {m.bestMove}</span>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
