"use client";

import React from "react";
import { FaUser, FaTrophy } from "react-icons/fa";
import { RiAliensFill } from "react-icons/ri";
import Lottie from "lottie-react";
import chessAnimation from "@/public/assets/chess.json";
import { useRouter } from "next/navigation";

interface GameModeButtonsProps {
  setGameMode: (mode: "online" | "bot" | null) => void;
}

const GameModeButtons: React.FC<GameModeButtonsProps> = ({ setGameMode }) => {
  const router = useRouter();

  return (
    <>
      {/* Lottie Animation */}
      <div className="w-full max-w-[300px] mx-auto">
        <Lottie
          animationData={chessAnimation}
          loop={true}
          autoplay={true}
          className="w-full h-auto"
          aria-hidden="true"
          style={{
            filter: "drop-shadow(0 4px 6px rgba(0, 0, 0, 0.3))",
            transform: "scale(1.1)",
          }}
        />
      </div>

      <div className="play-button-container relative">
        <button
          onClick={() => setGameMode("online")}
          aria-label="Play Online — Play with someone at your level"
          className="play-button w-full p-4 relative border-none outline-none cursor-pointer transition-all duration-300 ease-in-out z-[2] overflow-hidden flex items-center justify-center bg-transparent"
        >
          <span className="button-text flex items-center gap-6 z-[3]">
            <div className="bg-teal-400/20 p-3 rounded-full shadow-lg">
              <FaUser className="text-3xl text-white filter drop-shadow-md" />
            </div>
            <div className="text-left">
              <h3 className="text-lg font-bold text-white mb-0.5">
                Play Online
              </h3>
              <p className="text-green-100 text-xs">
                Play with someone at your level
              </p>
            </div>
          </span>
          <div className="clip absolute inset-0 border-[5px] border-solid border-transparent bg-gradient-to-r from-teal-500 to-blue-700 transition-all duration-300 ease-in-out" />
          <div className="corner top-left absolute w-[30px] h-[30px] bg-teal-500" />
          <div className="corner top-right absolute w-[30px] h-[30px] bg-blue-700" />
          <div className="corner bottom-left absolute w-[30px] h-[30px] bg-teal-500" />
          <div className="corner bottom-right absolute w-[30px] h-[30px] bg-blue-700" />
        </button>
      </div>

      <div className="play-button-container relative">
        <button
          onClick={() => setGameMode("bot")}
          aria-label="Play Bots — Play vs bot opponents for training"
          className="play-button w-full p-4 relative border-none outline-none cursor-pointer transition-all duration-300 ease-in-out z-[2] overflow-hidden flex items-center justify-center bg-transparent"
        >
          <span className="button-text flex items-center gap-6 z-[3]">
            <div className="bg-[#008e90]/20 p-3 rounded-full shadow-lg">
              <RiAliensFill className="text-3xl text-white filter drop-shadow-md" />
            </div>
            <div className="text-left">
              <h3 className="text-lg font-bold text-white mb-0.5">Play Bots</h3>
              <p className="text-gray-300 text-xs">
                Play vs customizable training bots
              </p>
            </div>
          </span>
          <div className="clip absolute inset-0 border-[5px] border-solid border-transparent bg-[#008e90] transition-all duration-300 ease-in-out" />
          <div className="corner top-left absolute w-[30px] h-[30px] bg-[#008e90]" />
          <div className="corner top-right absolute w-[30px] h-[30px] bg-[#008e90]" />
          <div className="corner bottom-left absolute w-[30px] h-[30px] bg-[#008e90]" />
          <div className="corner bottom-right absolute w-[30px] h-[30px] bg-[#008e90]" />
        </button>
      </div>

      <div className="play-button-container relative">
        <button
          onClick={() => router.push("/puzzles")}
          aria-label="Learn and Earn — Solve puzzles to earn XLM rewards"
          className="play-button w-full p-4 relative border-none outline-none cursor-pointer transition-all duration-300 ease-in-out z-[2] overflow-hidden flex items-center justify-center bg-transparent"
        >
          <span className="button-text flex items-center gap-6 z-[3]">
            <div className="bg-yellow-400/20 p-3 rounded-full shadow-lg">
              <FaTrophy className="text-3xl text-white filter drop-shadow-md" />
            </div>
            <div className="text-left">
              <h3 className="text-lg font-bold text-white mb-0.5">
                Learn & Earn
              </h3>
              <p className="text-yellow-100 text-xs">
                Solve puzzles to earn XLM rewards
              </p>
            </div>
          </span>
          <div className="clip absolute inset-0 border-[5px] border-solid border-transparent bg-gradient-to-r from-yellow-500 to-orange-600 transition-all duration-300 ease-in-out" />
          <div className="corner top-left absolute w-[30px] h-[30px] bg-yellow-500" />
          <div className="corner top-right absolute w-[30px] h-[30px] bg-orange-600" />
          <div className="corner bottom-left absolute w-[30px] h-[30px] bg-yellow-500" />
          <div className="corner bottom-right absolute w-[30px] h-[30px] bg-orange-600" />
        </button>
      </div>
      <style jsx>{`
        .play-button-container * {
          margin: 0;
          padding: 0;
          box-sizing: border-box;
        }
        .play-button {
          font-size: 18px;
          font-weight: bold;
          letter-spacing: 2px;
          text-transform: uppercase;
          min-height: 80px;
        }
        .play-button:hover {
          transform: scale(1.05);
        }
        .play-button:active {
          transform: scale(0.95);
        }
        .clip {
          clip-path: polygon(
            20% 0%,
            80% 0%,
            100% 30%,
            100% 70%,
            80% 100%,
            20% 100%,
            0% 70%,
            0% 30%
          );
          box-shadow: inset 0 0 15px rgba(0, 0, 0, 0.6);
          animation: shape-glitch 1.5s infinite ease-in-out;
        }
        .play-button:hover .clip {
          box-shadow: inset 0 0 25px rgba(255, 255, 255, 0.2);
        }
        .corner {
          transform: rotate(45deg);
          transition: 0.2s ease;
          animation: corner-glitch 2s infinite ease-in-out;
          box-shadow: inset 1px 1px 8px rgba(255, 255, 255, 0.3);
        }
        .top-left {
          top: -15px;
          left: -15px;
        }
        .top-right {
          top: -15px;
          right: -15px;
        }
        .bottom-left {
          bottom: -15px;
          left: -15px;
        }
        .bottom-right {
          bottom: -15px;
          right: -15px;
        }
        .play-button:hover .corner {
          transform: scale(1.25) rotate(45deg);
          animation: corner-light 0.4s ease-in-out infinite alternate;
        }
        @keyframes shape-glitch {
          0%,
          100% {
            clip-path: polygon(
              20% 0%,
              80% 0%,
              100% 30%,
              100% 70%,
              80% 100%,
              20% 100%,
              0% 70%,
              0% 30%
            );
            opacity: 0.9;
          }
          25% {
            clip-path: polygon(
              15% 0%,
              85% 0%,
              100% 35%,
              100% 65%,
              85% 100%,
              15% 100%,
              0% 65%,
              0% 35%
            );
            opacity: 0.95;
          }
          50% {
            clip-path: polygon(
              10% 0%,
              90% 0%,
              100% 40%,
              100% 60%,
              90% 100%,
              10% 100%,
              0% 60%,
              0% 40%
            );
            opacity: 1;
          }
        }
        @keyframes corner-glitch {
          0%,
          100% {
            transform: scale(1) rotate(45deg);
            opacity: 1;
          }
          25% {
            transform: scale(1.1) rotate(50deg);
            opacity: 0.9;
          }
          50% {
            transform: scale(0.9) rotate(40deg);
            opacity: 0.8;
          }
          75% {
            transform: scale(1.1) rotate(50deg);
            opacity: 0.9;
          }
        }
        @keyframes corner-light {
          0% {
            box-shadow: inset 0 0 5px rgba(255, 255, 255, 0.3);
          }
          100% {
            box-shadow: inset 0 0 15px rgba(255, 255, 255, 0.5);
          }
        }
      `}</style>
    </>
  );
};
export default GameModeButtons;
