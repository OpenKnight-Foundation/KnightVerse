import React, { useState, useEffect } from 'react';

export const PuzzleRush: React.FC = () => {
  const [timeLeft, setTimeLeft] = useState(300);
  const [combo, setCombo] = useState(0);
  const [strikes, setStrikes] = useState(0);
  const [gameOver, setGameOver] = useState(false);

  useEffect(() => {
    const timer = setInterval(() => {
      setTimeLeft(t => {
        if (t <= 1) { setGameOver(true); return 0; }
        return t - 1;
      });
    }, 1000);
    return () => clearInterval(timer);
  }, []);

  const handleCorrectSolve = () => setCombo(c => c + 1);
  const handleIncorrectMove = () => {
    setStrikes(s => {
      const ns = s + 1;
      if (ns >= 3) setGameOver(true);
      return ns;
    });
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gradient-to-b from-blue-900 to-black">
      <div className="text-6xl font-bold text-white mb-8">{Math.floor(timeLeft / 60)}:{String(timeLeft % 60).padStart(2, '0')}</div>
      <div className="text-4xl font-bold text-yellow-400 mb-6">Combo: {combo}x</div>
      <div className="flex gap-4 mb-8">
        {[0,1,2].map(i => <div key={i} className={`w-8 h-8 rounded-full ${i < strikes ? 'bg-red-600' : 'bg-gray-400'}`} />)}
      </div>
      {gameOver && <div className="bg-white p-8 rounded"><h2 className="text-2xl font-bold mb-4">Game Over</h2><p>Puzzles Solved: {combo}</p></div>}
    </div>
  );
};
