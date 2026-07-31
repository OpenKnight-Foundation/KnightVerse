"use client";

import React, { useState, useEffect, useMemo, useCallback, useRef } from "react";
import Image from "next/image";
import { useBoardTheme } from "@/context/ThemeContext";

import WhiteKing from "./chesspieces/white-king.svg";
import WhiteQueen from "./chesspieces/white-queen.svg";
import WhiteBishop from "./chesspieces/white-bishop.svg";
import WhiteKnight from "./chesspieces/white-knight.svg";
import WhiteRook from "./chesspieces/white-rook.svg";
import WhitePawn from "./chesspieces/white-pawn.svg";
import BlackKing from "./chesspieces/black-king.svg";
import BlackQueen from "./chesspieces/black-queen.svg";
import BlackBishop from "./chesspieces/black-bishop.svg";
import BlackKnight from "./chesspieces/black-knight.svg";
import BlackRook from "./chesspieces/black-rook.svg";
import BlackPawn from "./chesspieces/black-pawn.svg";

interface ChessboardComponentProps {
  position: string;
  onDrop: (params: { sourceSquare: string; targetSquare: string }) => boolean;
  width?: number; // Added width as optional prop
  orientation?: "white" | "black"; // Board orientation: white = normal, black = flipped
}

// Parse FEN string to board state - memoized pure function
const parseFen = (fen: string): string[][] => {
  if (fen === "start") {
    return [
      ["bR", "bN", "bB", "bQ", "bK", "bB", "bN", "bR"],
      ["bP", "bP", "bP", "bP", "bP", "bP", "bP", "bP"],
      ["", "", "", "", "", "", "", ""],
      ["", "", "", "", "", "", "", ""],
      ["", "", "", "", "", "", "", ""],
      ["", "", "", "", "", "", "", ""],
      ["wP", "wP", "wP", "wP", "wP", "wP", "wP", "wP"],
      ["wR", "wN", "wB", "wQ", "wK", "wB", "wN", "wR"],
    ];
  }

  try {
    const fenParts = fen.split(" ");
    const rows = fenParts[0].split("/");
    const newBoard: string[][] = [];

    rows.forEach((row) => {
      const newRow: string[] = [];
      for (let i = 0; i < row.length; i++) {
        const char = row[i];
        if (isNaN(parseInt(char))) {
          const color = char === char.toUpperCase() ? "w" : "b";
          newRow.push(`${color}${char.toUpperCase()}`);
        } else {
          for (let j = 0; j < parseInt(char); j++) {
            newRow.push("");
          }
        }
      }
      newBoard.push(newRow);
    });

    return newBoard;
  } catch (e) {
    console.error("Error parsing FEN:", e);
    return Array.from({ length: 8 }, () => Array(8).fill(""));
  }
};

// Format piece code into a human-readable name for screen readers
const formatPieceName = (piece: string): string => {
  if (!piece) return "";
  const color = piece[0] === "w" ? "white" : "black";
  const names: Record<string, string> = {
    K: "king",
    Q: "queen",
    R: "rook",
    B: "bishop",
    N: "knight",
    P: "pawn",
  };
  return `${color} ${names[piece[1]] ?? piece[1]}`;
};

// ChessboardComponent with full memoization
const ChessboardComponent: React.FC<ChessboardComponentProps> = ({
  position,
  onDrop,
  width,
  orientation = "white",
}) => {
  const [mounted, setMounted] = useState(false);
  const [boardWidth, setBoardWidth] = useState(width || 560);
  const [selectedSquare, setSelectedSquare] = useState<string | null>(null);
  const [hoveredSquare, setHoveredSquare] = useState<string | null>(null);
  const touchStartSquare = useRef<string | null>(null);
  const boardRef = useRef<HTMLDivElement>(null);
  
  const { colors } = useBoardTheme();

  // Memoize board state parsing - prevents re-parsing on every render
  const boardState = useMemo(() => parseFen(position), [position]);

  // When orientation is black, flip both rows and columns for display
  const displayRows = useMemo(() => {
    if (orientation === "black") {
      return [...boardState].reverse().map((row) => [...row].reverse());
    }
    return boardState;
  }, [boardState, orientation]);

  useEffect(() => {
    const updateBoardSize = () => {
      if (typeof document === "undefined") return;
      const container = document.querySelector(
        ".chessboard-container",
      )?.parentElement;
      if (!container) return;
      const vw = Math.max(
        document.documentElement.clientWidth || 0,
        window.innerWidth || 0,
      );
      const containerWidth = container.clientWidth;
      const maxSize = 560;
      const minSize = Math.min(320, containerWidth);
      let newWidth;
      if (vw < 768) {
        newWidth = Math.max(minSize, Math.min(containerWidth * 0.95, maxSize));
      } else {
        newWidth = Math.min(containerWidth, maxSize);
      }

      setBoardWidth(newWidth);
    };

    if (mounted) {
      updateBoardSize();
      window.addEventListener("resize", updateBoardSize);
      window.addEventListener("orientationchange", updateBoardSize);
    }

    return () => {
      window.removeEventListener("resize", updateBoardSize);
      window.removeEventListener("orientationchange", updateBoardSize);
    };
  }, [mounted]);

  useEffect(() => {
    setMounted(true);
  }, []);

  // Memoize piece image mapping - prevents recreation on every render
  const pieceImages: Record<string, string> = useMemo(
    () => ({
      wP: WhitePawn,
      wR: WhiteRook,
      wN: WhiteKnight,
      wB: WhiteBishop,
      wQ: WhiteQueen,
      wK: WhiteKing,
      bP: BlackPawn,
      bR: BlackRook,
      bN: BlackKnight,
      bB: BlackBishop,
      bQ: BlackQueen,
      bK: BlackKing,
    }),
    [],
  );

  const getPieceImage = useCallback(
    (piece: string) => {
      if (!piece) return null;
      const isWhite = piece.startsWith("w");
      return (
        <div
          className="piece-container group"
          style={{
            width: "100%",
            height: "100%",
            display: "flex",
            justifyContent: "center",
            alignItems: "center",
            position: "relative",
            userSelect: "none",
            cursor: "grab",
            pointerEvents: "none",
            transform: `scale(${boardWidth < 400 ? 0.7 : 0.9})`,
            transition: "all 0.2s ease",
          }}
        >
          <div
            style={{
              width: boardWidth < 400 ? "80%" : "90%",
              height: boardWidth < 400 ? "80%" : "90%",
              position: "relative",
              transform: "scale(1)",
              transition: "transform 0.2s ease",
              aspectRatio: "1/1",
              minHeight: "40px",
            }}
            className="group-hover:transform group-hover:scale-110"
          >
            <Image
              src={pieceImages[piece]}
              alt={piece}
              fill
              priority
              sizes="(max-width: 400px) 80vw, 90vw"
              style={{
                width: "100%",
                height: "100%",
                objectFit: "contain",
                filter: isWhite
                  ? "drop-shadow(2px 2px 2px rgba(0,0,0,0.5))"
                  : "drop-shadow(2px 2px 2px rgba(0,0,0,0.3))",
                transition: "filter 0.2s ease",
              }}
              className="group-hover:filter group-hover:brightness-110"
              onError={(e) => {
                console.error(`Failed to load chess piece: ${piece}`);
                const target = e.target as HTMLImageElement;
                if (target) {
                  target.style.opacity = "0.5";
                }
              }}
            />
          </div>
        </div>
      );
    },
    [boardWidth, pieceImages],
  );

  const attemptMove = useCallback(
    (
      sourceRow: number,
      sourceCol: number,
      targetRow: number,
      targetCol: number,
    ): void => {
      // Map display indices back to actual board indices when flipped
      const toActual = (row: number, col: number) =>
        orientation === "black" ? [7 - row, 7 - col] : [row, col];

      const [actualSrcRow, actualSrcCol] = toActual(sourceRow, sourceCol);
      const [actualTgtRow, actualTgtCol] = toActual(targetRow, targetCol);

      const sourceSquare = `${String.fromCharCode(97 + actualSrcCol)}${
        8 - actualSrcRow
      }`;
      const targetSquare = `${String.fromCharCode(97 + actualTgtCol)}${
        8 - actualTgtRow
      }`;
      const moveSuccess = onDrop({ sourceSquare, targetSquare });
      if (moveSuccess) {
        setSelectedSquare(null);
      }
    },
    [onDrop, orientation],
  );

  const handleSquareClick = useCallback(
    (row: number, col: number) => {
      const clickedSquare = `${row},${col}`;
      const clickedPiece = displayRows[row][col];

      // No piece selected yet — select if there's a piece on the square.
      if (!selectedSquare && clickedPiece) {
        setSelectedSquare(clickedSquare);
        return;
      }

      // Clicking the already-selected square deselects it.
      if (selectedSquare === clickedSquare) {
        setSelectedSquare(null);
        return;
      }

      const [sourceRow, sourceCol] = selectedSquare.split(",").map(Number);
      const selectedPiece = boardState[sourceRow][sourceCol];

      // If the target square holds a piece of the same color, switch selection
      // rather than attempting an illegal capture.
      if (
        clickedPiece &&
        selectedPiece &&
        clickedPiece[0] === selectedPiece[0] // same color prefix ('w' or 'b')
      ) {
        setSelectedSquare(clickedSquare);
        return;
      }

      // Otherwise attempt the move; clear selection only on success.
      attemptMove(sourceRow, sourceCol, row, col);
    },
    [selectedSquare, boardState, displayRows, attemptMove],
  );

  const handleDragStart = useCallback(
    (e: React.DragEvent, row: number, col: number) => {
      e.dataTransfer.setData("text/plain", `${row},${col}`);
      const draggedElement = e.currentTarget as HTMLElement;
      if (draggedElement) {
        draggedElement.style.opacity = "0.6";
      }
    },
    [],
  );

  const handleDragEnd = useCallback((e: React.DragEvent) => {
    const draggedElement = e.currentTarget as HTMLElement;
    if (draggedElement) {
      draggedElement.style.opacity = "1";
    }
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, targetRow: number, targetCol: number) => {
      e.preventDefault();
      const data = e.dataTransfer.getData("text/plain");
      const [sourceRow, sourceCol] = data.split(",").map(Number);
      attemptMove(sourceRow, sourceCol, targetRow, targetCol);
    },
    [attemptMove],
  );

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!selectedSquare) return;
      const [row, col] = selectedSquare.split(",").map(Number);
      let nextRow = row;
      let nextCol = col;

      switch (e.key) {
        case "ArrowUp":
          e.preventDefault();
          nextRow = Math.max(0, row - 1);
          break;
        case "ArrowDown":
          e.preventDefault();
          nextRow = Math.min(7, row + 1);
          break;
        case "ArrowLeft":
          e.preventDefault();
          nextCol = Math.max(0, col - 1);
          break;
        case "ArrowRight":
          e.preventDefault();
          nextCol = Math.min(7, col + 1);
          break;
        case "Escape":
          setSelectedSquare(null);
          return;
        default:
          return;
      }

      const nextKey = `${nextRow},${nextCol}`;
      setSelectedSquare(nextKey);
      const nextCell = boardRef.current?.querySelector(
        `[data-square="${nextKey}"]`,
      ) as HTMLElement | null;
      nextCell?.focus();
    },
    [selectedSquare],
  );

  const getSquareFromTouch = useCallback((touch: React.Touch): [number, number] | null => {
    if (!boardRef.current) return null;
    const rect = boardRef.current.getBoundingClientRect();
    const x = touch.clientX - rect.left;
    const y = touch.clientY - rect.top;
    const col = Math.floor((x / rect.width) * 8);
    const row = Math.floor((y / rect.height) * 8);
    if (col < 0 || col > 7 || row < 0 || row > 7) return null;
    return [row, col];
  }, []);

  const handleTouchStart = useCallback(
    (e: React.TouchEvent, row: number, col: number) => {
      if (!displayRows[row][col]) return;
      touchStartSquare.current = `${row},${col}`;
      setSelectedSquare(`${row},${col}`);
    },
    [displayRows],
  );

  const handleTouchMove = useCallback(
    (e: React.TouchEvent) => {
      if (!touchStartSquare.current) return;
      const sq = getSquareFromTouch(e.touches[0]);
      if (sq) setHoveredSquare(`${sq[0]},${sq[1]}`);
    },
    [getSquareFromTouch],
  );

  const handleTouchEnd = useCallback(
    (e: React.TouchEvent) => {
      if (!touchStartSquare.current) return;
      const sq = getSquareFromTouch(e.changedTouches[0]);
      if (sq) {
        const [srcRow, srcCol] = touchStartSquare.current.split(",").map(Number);
        attemptMove(srcRow, srcCol, sq[0], sq[1]);
      }
      touchStartSquare.current = null;
      setHoveredSquare(null);
      setSelectedSquare(null);
    },
    [getSquareFromTouch, attemptMove],
  );

  if (!mounted) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-gray-800 rounded-md">
        <div className="text-white">Initializing chessboard...</div>
      </div>
    );
  }
  return (
    <div
      ref={boardRef}
      className="chessboard-container w-full mx-auto relative"
      role="grid"
      aria-label={`Chess board, ${orientation === "white" ? "White" : "Black"} perspective`}
      aria-roledescription="chessboard"
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      onKeyDown={handleKeyDown}
      style={{
        width: "100%",
        maxWidth: `${boardWidth}px`,
        minWidth: "min(280px, 90vw)",
        aspectRatio: "1/1",
        display: "grid",
        gridTemplateColumns: `repeat(8, minmax(0, 1fr))`,
        gridTemplateRows: `repeat(8, minmax(0, 1fr))`,
        border: "2px solid #005dad",
        borderRadius: "4px",
        boxShadow: "0 8px 16px rgba(0, 93, 173, 0.3)",
        overflow: "visible",
        touchAction: "none",
        margin: "0 auto",
        padding: "1%",
        transform: "scale(var(--board-scale, 1))",
        transformOrigin: "center center",
      }}
      aria-live="polite"
    >
      {/* Screen reader live region for move announcements */}
      <div className="sr-only" role="status" aria-live="polite" aria-atomic="true">
        {selectedSquare && (() => {
          const [r, c] = selectedSquare.split(",").map(Number);
          const actualR = orientation === "black" ? 7 - r : r;
          const actualC = orientation === "black" ? 7 - c : c;
          const sq = `${String.fromCharCode(97 + actualC)}${8 - actualR}`;
          const piece = displayRows[r][c];
          return `Selected ${piece} on ${sq}`;
        })()}
      </div>
      {displayRows.map((row, rowIndex) =>
        row.map((piece, colIndex) => {
          const isLight = (rowIndex + colIndex) % 2 === 1;
          const squareKey = `${rowIndex},${colIndex}`;
          const isSelected = selectedSquare === squareKey;
          const isHovered = hoveredSquare === squareKey && hoveredSquare !== selectedSquare;

          // Compute actual board coordinates for the aria-label
          const actualRow = orientation === "black" ? 7 - rowIndex : rowIndex;
          const actualCol = orientation === "black" ? 7 - colIndex : colIndex;
          const squareLabel = `${String.fromCharCode(97 + actualCol)}${8 - actualRow}`;

          return (
            <div
              key={`${rowIndex}-${colIndex}`}
              data-square={`${rowIndex}-${colIndex}`}
              role="gridcell"
              aria-label={`${squareLabel}${piece ? ", " + formatPieceName(piece) : ", empty"}`}
              aria-selected={isSelected}
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleSquareClick(rowIndex, colIndex);
                }
              }}
              style={{
                backgroundColor: isLight ? colors.dark : colors.light,
                width: "100%",
                height: "100%",
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                cursor: piece ? "grab" : "default",
                position: "relative",
                boxShadow: isSelected
                  ? "inset 0 0 0 3px rgba(0, 93, 173, 0.75)"
                  : isHovered
                  ? "inset 0 0 0 3px rgba(0, 200, 170, 0.7)"
                  : "none",
                transition: "background-color 0.2s ease, box-shadow 0.1s ease",
              }}
              onClick={() => handleSquareClick(rowIndex, colIndex)}
              onTouchStart={(e) => handleTouchStart(e, rowIndex, colIndex)}
              draggable={!!piece}
              onDragStart={(e) => handleDragStart(e, rowIndex, colIndex)}
              onDragEnd={handleDragEnd}
              onDrop={(e) => handleDrop(e, rowIndex, colIndex)}
              onDragOver={handleDragOver}
            >
              {piece && (
                <div
                  style={{
                    transition: "transform 0.2s ease-out",
                    transform: `scale(${isSelected ? 1.1 : 1})`,
                  }}
                >
                  {getPieceImage(piece)}
                </div>
              )}
            </div>
          );
        }),
      )}
    </div>
  );
};

export default React.memo(ChessboardComponent);
