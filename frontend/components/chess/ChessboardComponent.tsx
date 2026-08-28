"use client";

import React, {
  useState,
  useEffect,
  useMemo,
  useCallback,
  useRef,
} from "react";
import Image from "next/image";
import { useBoardTheme } from "@/context/ThemeContext";
import { Chess } from "chess.js";

function parseFen(fen: string): (string | null)[][] {
  const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
  const validFen = !fen || fen === "start" ? START_FEN : fen;
  try {
    const board = new Chess(validFen);
    return board.board().map((row) =>
      row.map((cell) => {
        if (!cell) return null;
        const color = cell.color === "w" ? "w" : "b";
        const piece = cell.type.toUpperCase();
        return `${color}${piece}`;
      }),
    );
  } catch {
    const board = new Chess(START_FEN);
    return board.board().map((row) =>
      row.map((cell) => {
        if (!cell) return null;
        const color = cell.color === "w" ? "w" : "b";
        const piece = cell.type.toUpperCase();
        return `${color}${piece}`;
      }),
    );
  }
}

function formatPieceName(piece: string): string {
  const pieceMap: Record<string, string> = {
    wP: "White Pawn",
    wR: "White Rook",
    wN: "White Knight",
    wB: "White Bishop",
    wQ: "White Queen",
    wK: "White King",
    bP: "Black Pawn",
    bR: "Black Rook",
    bN: "Black Knight",
    bB: "Black Bishop",
    bQ: "Black Queen",
    bK: "Black King",
  };
  return pieceMap[piece] || "Unknown Piece";
}

import { useGamePreferences } from "@/context/GamePreferencesContext";

// Import standard piece set assets
import StandardWhiteKing from "./chesspieces/white-king.svg";
import StandardWhiteQueen from "./chesspieces/white-queen.svg";
import StandardWhiteBishop from "./chesspieces/white-bishop.svg";
import StandardWhiteKnight from "./chesspieces/white-knight.svg";
import StandardWhiteRook from "./chesspieces/white-rook.svg";
import StandardWhitePawn from "./chesspieces/white-pawn.svg";
import StandardBlackKing from "./chesspieces/black-king.svg";
import StandardBlackQueen from "./chesspieces/black-queen.svg";
import StandardBlackBishop from "./chesspieces/black-bishop.svg";
import StandardBlackKnight from "./chesspieces/black-knight.svg";
import StandardBlackRook from "./chesspieces/black-rook.svg";
import StandardBlackPawn from "./chesspieces/black-pawn.svg";

// Import all Neo piece set assets
import NeoWhiteKing from "./chesspieces/neo/white-king.svg";
import NeoWhiteQueen from "./chesspieces/neo/white-queen.svg";
import NeoWhiteBishop from "./chesspieces/neo/white-bishop.svg";
import NeoWhiteKnight from "./chesspieces/neo/white-knight.svg";
import NeoWhiteRook from "./chesspieces/neo/white-rook.svg";
import NeoWhitePawn from "./chesspieces/neo/white-pawn.svg";
import NeoBlackKing from "./chesspieces/neo/black-king.svg";
import NeoBlackQueen from "./chesspieces/neo/black-queen.svg";
import NeoBlackBishop from "./chesspieces/neo/black-bishop.svg";
import NeoBlackKnight from "./chesspieces/neo/black-knight.svg";
import NeoBlackRook from "./chesspieces/neo/black-rook.svg";
import NeoBlackPawn from "./chesspieces/neo/black-pawn.svg";

// Import custom King piece set assets
import StauntonWhiteKing from "./chesspieces/staunton/white-king.svg";
import StauntonBlackKing from "./chesspieces/staunton/black-king.svg";

import AlphaWhiteKing from "./chesspieces/alpha/white-king.svg";
import AlphaBlackKing from "./chesspieces/alpha/black-king.svg";

import MedievalWhiteKing from "./chesspieces/medieval/white-king.svg";
import MedievalBlackKing from "./chesspieces/medieval/black-king.svg";

import CyberpunkWhiteKing from "./chesspieces/cyberpunk/white-king.svg";
import CyberpunkBlackKing from "./chesspieces/cyberpunk/black-king.svg";

import { PremoveService, PreMove } from "@/services/premoveService";
import PremoveArrow from "./PremoveArrow";

interface ChessboardComponentProps {
  position: string;
  onDrop: (params: { sourceSquare: string; targetSquare: string }) => boolean | Promise<boolean>;
  width?: number; // Added width as optional prop
  orientation?: "white" | "black"; // Board orientation: white = normal, black = flipped
  lastMove?: { from: string; to: string } | [string, string] | null;
  isMyTurn?: boolean;
  "aria-label"?: string;
}

const ChessboardComponent: React.FC<ChessboardComponentProps> = ({
  position,
  onDrop,
  orientation = "white",
  lastMove,
  "aria-label": ariaLabel,
}) => {
  const { preferences } = useGamePreferences();
  const [premoves, setPremoves] = useState<PreMove[]>([]);
  const premoveService = useRef(new PremoveService());

  const clearPremoves = (e: React.MouseEvent) => {
    e.preventDefault();
    premoveService.current.clearPremoves();
    setPremoves([]);
  };

  const [mounted, setMounted] = useState(typeof window !== "undefined");
  const [boardWidth] = useState(560);
  const [selectedSquare, setSelectedSquare] = useState<string | null>(null);
  const [hoveredSquare, setHoveredSquare] = useState<string | null>(null);
  const [focusedSquare, setFocusedSquare] = useState<string | null>(null);
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
    const { executedMove } =
      premoveService.current.handleOpponentMove(position);
    if (executedMove) {
      onDrop({
        sourceSquare: executedMove.from,
        targetSquare: executedMove.to,
      });
    }
    setPremoves(premoveService.current.getPremoves());
  }, [position, onDrop]);

  useEffect(() => {
    setMounted(true);
  }, []);

  // Memoize piece image mapping - prevents recreation on every render
  const pieceImages: Record<string, string> = useMemo(() => {
    // Piece set assets mapping
    const standardPieces = {
      wP: StandardWhitePawn,
      wR: StandardWhiteRook,
      wN: StandardWhiteKnight,
      wB: StandardWhiteBishop,
      wQ: StandardWhiteQueen,
      wK: StandardWhiteKing,
      bP: StandardBlackPawn,
      bR: StandardBlackRook,
      bN: StandardBlackKnight,
      bB: StandardBlackBishop,
      bQ: StandardBlackQueen,
      bK: StandardBlackKing,
    };

    const pieceSetAssets: Record<string, Record<string, string>> = {
      neo: {
        wP: NeoWhitePawn,
        wR: NeoWhiteRook,
        wN: NeoWhiteKnight,
        wB: NeoWhiteBishop,
        wQ: NeoWhiteQueen,
        wK: NeoWhiteKing,
        bP: NeoBlackPawn,
        bR: NeoBlackRook,
        bN: NeoBlackKnight,
        bB: NeoBlackBishop,
        bQ: NeoBlackQueen,
        bK: NeoBlackKing,
      },
      staunton: {
        ...standardPieces,
        wK: StauntonWhiteKing,
        bK: StauntonBlackKing,
      },
      alpha: {
        ...standardPieces,
        wK: AlphaWhiteKing,
        bK: AlphaBlackKing,
      },
      medieval: {
        ...standardPieces,
        wK: MedievalWhiteKing,
        bK: MedievalBlackKing,
      },
      cyberpunk: {
        ...standardPieces,
        wK: CyberpunkWhiteKing,
        bK: CyberpunkBlackKing,
      },
    };
    
    return pieceSetAssets[preferences.pieceSet] || pieceSetAssets.neo;
  }, [preferences.pieceSet]);

  const getPieceImage = useCallback(
    (piece: string) => {
      if (!piece) return null;
      const isWhite = piece.startsWith("w");
      return (
        <div
          className="piece-container group will-change-transform"
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
            transition: "transform 0.15s cubic-bezier(0.4, 0, 0.2, 1), filter 0.15s ease",
            backfaceVisibility: "hidden",
            WebkitBackfaceVisibility: "hidden",
          }}
        >
          <div
            style={{
              width: boardWidth < 400 ? "80%" : "90%",
              height: boardWidth < 400 ? "80%" : "90%",
              position: "relative",
              transform: "scale(1)",
              transition: "transform 0.15s cubic-bezier(0.4, 0, 0.2, 1)",
              aspectRatio: "1/1",
              minHeight: "40px",
              backfaceVisibility: "hidden",
              WebkitBackfaceVisibility: "hidden",
            }}
            className="group-hover:transform group-hover:scale-110 will-change-transform"
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

      // No square selected (and clicked square was empty) — nothing to do.
      if (!selectedSquare) return;

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

  const [highlightedSquares, setHighlightedSquares] = useState<
    { square: string; color: string }[]
  >([]);

  const handleRightClick = (e: React.MouseEvent, row: number, col: number) => {
    e.preventDefault();
    const square = `${row},${col}`;
    const color = e.shiftKey
      ? "rgba(239, 68, 68, 0.6)"
      : e.altKey
        ? "rgba(59, 130, 246, 0.6)"
        : e.ctrlKey
          ? "rgba(234, 179, 8, 0.6)"
          : "rgba(34, 197, 94, 0.6)";

    setHighlightedSquares((prev) =>
      prev.some((s) => s.square === square)
        ? prev.filter((s) => s.square !== square)
        : [...prev, { square, color }],
    );
  };

  const handleDragStart = (e: React.DragEvent, row: number, col: number) => {
    e.dataTransfer.setData("text/plain", `${row},${col}`);
  };

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
  }, []);

  const handleDragEnd = useCallback(() => {
    setHoveredSquare(null);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, targetRow: number, targetCol: number) => {
      e.preventDefault();
      const data = e.dataTransfer.getData("text/plain");
      if (!data) return;
      const [sourceRow, sourceCol] = data.split(",").map(Number);
      attemptMove(sourceRow, sourceCol, targetRow, targetCol);
    },
    [attemptMove],
  );

  const focusSquare = useCallback((row: number, col: number) => {
    const key = `${row},${col}`;
    setFocusedSquare(key);
    const nextCell = boardRef.current?.querySelector(
      `[data-square="${row}-${col}"]`,
    ) as HTMLElement | null;
    nextCell?.focus();
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Determine the anchor: use focusedSquare if set, otherwise default to 0,0
      const anchor = focusedSquare ?? selectedSquare ?? "0,0";
      const [row, col] = anchor.split(",").map(Number);
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
          e.preventDefault();
          setSelectedSquare(null);
          setFocusedSquare(null);
          return;
        default:
          return;
      }

      focusSquare(nextRow, nextCol);
    },
    [focusedSquare, selectedSquare, focusSquare],
  );

  const getSquareFromTouch = useCallback(
    (touch: React.Touch): [number, number] | null => {
      if (!boardRef.current) return null;
      const rect = boardRef.current.getBoundingClientRect();
      const x = touch.clientX - rect.left;
      const y = touch.clientY - rect.top;
      const col = Math.floor((x / rect.width) * 8);
      const row = Math.floor((y / rect.height) * 8);
      if (col < 0 || col > 7 || row < 0 || row > 7) return null;
      return [row, col];
    },
    [],
  );

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
        const [srcRow, srcCol] = touchStartSquare.current
          .split(",")
          .map(Number);
        attemptMove(srcRow, srcCol, sq[0], sq[1]);
      }
      touchStartSquare.current = null;
      setHoveredSquare(null);
      setSelectedSquare(null);
    },
    [getSquareFromTouch, attemptMove],
  );

  const renderGhostPiece = (piece: string, square: string) => {
    const pieceImage = getPieceImage(piece);
    if (!pieceImage) return null;

    const [row, col] = [parseInt(square[1]) - 1, square.charCodeAt(0) - 97];

    return (
      <div
        style={{
          position: "absolute",
          top: `${row * 12.5}%`,
          left: `${col * 12.5}%`,
          width: "12.5%",
          height: "12.5%",
          opacity: 0.5,
          pointerEvents: "none",
        }}
      >
        {pieceImage}
      </div>
    );
  };

  if (!mounted) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-gray-800 rounded-md">
        <div className="text-white">Initializing chessboard...</div>
      </div>
    );
  }
  return (
    <div
      className="chessboard-wrapper w-full mx-auto relative"
      style={{ maxWidth: `${boardWidth}px` }}
    >
      {/* Screen reader live region for move announcements — must be outside role="grid" */}
      <div
        className="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {selectedSquare &&
          (() => {
            const [r, c] = selectedSquare.split(",").map(Number);
            const actualR = orientation === "black" ? 7 - r : r;
            const actualC = orientation === "black" ? 7 - c : c;
            const sq = `${String.fromCharCode(97 + actualC)}${8 - actualR}`;
            const piece = displayRows[r][c];
            return piece
              ? `Selected ${formatPieceName(piece)} on ${sq}`
              : `Focused ${sq}`;
          })()}
      </div>
      <div
        ref={boardRef}
        className="chessboard-container w-full mx-auto relative"
        role="grid"
        aria-label={ariaLabel || `Chess board, ${orientation === "white" ? "White" : "Black"} perspective`}
        aria-roledescription="chessboard"
        onTouchMove={handleTouchMove}
        onTouchEnd={handleTouchEnd}
        onKeyDown={handleKeyDown}
        onContextMenu={clearPremoves}
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
      >
        {premoves.map((premove, index) => (
          <PremoveArrow
            key={`arrow-${index}-${premove.from}-${premove.to}`}
            from={premove.from}
            to={premove.to}
            color={index === 0 ? "#3b82f6" : "#a855f7"}
          />
        ))}
        {premoves.map((premove) => {
          const toSquare = premove.to;
          const piece = premove.piece;
          return renderGhostPiece(piece, toSquare);
        })}
        {/* Screen reader live region for selection announcements */}
        <div
          className="sr-only"
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {selectedSquare &&
            (() => {
              const [r, c] = selectedSquare.split(",").map(Number);
              const actualR = orientation === "black" ? 7 - r : r;
              const actualC = orientation === "black" ? 7 - c : c;
              const sq = `${String.fromCharCode(97 + actualC)}${8 - actualR}`;
              const piece = displayRows[r][c];
              return `Selected ${piece} on ${sq}. Use arrow keys to navigate, Space to move, Escape to deselect.`;
            })()}
        </div>
        {displayRows.map((row, rowIndex) =>
          row.map((piece, colIndex) => {
            const isLight = (rowIndex + colIndex) % 2 === 1;
            const squareKey = `${rowIndex},${colIndex}`;
            const isSelected = selectedSquare === squareKey;
            const isFocused = focusedSquare === squareKey;
            const isHovered =
              hoveredSquare === squareKey && hoveredSquare !== selectedSquare;
            const highlightedSquare = highlightedSquares.find(
              (s) => s.square === squareKey,
            );

          // Compute actual board coordinates for the aria-label
          const actualRow = orientation === "black" ? 7 - rowIndex : rowIndex;
          const actualCol = orientation === "black" ? 7 - colIndex : colIndex;
          const squareLabel = `${String.fromCharCode(97 + actualCol)}${8 - actualRow}`;
          const isLastMoveSquare = Boolean(
            lastMove &&
              (Array.isArray(lastMove)
                ? lastMove.includes(squareLabel)
                : lastMove.from === squareLabel || lastMove.to === squareLabel)
          );

            const selectionHint = isSelected
              ? ". Piece selected. Press Space on another square to move, or Escape to deselect."
              : "";
            const focusHint =
              isFocused && !isSelected
                ? ". Press Space to select this piece."
                : "";

          const selectedShadow = colors.selected
            ? `inset 0 0 0 3px ${colors.selected}`
            : "inset 0 0 0 3px rgba(0, 93, 173, 0.75)";
          const lastMoveShadow = colors.lastMove
            ? `inset 0 0 0 3px ${colors.lastMove}`
            : "inset 0 0 0 2px rgba(245, 246, 130, 0.75)";

          return (
            <div
              key={`${rowIndex}-${colIndex}`}
              data-square={`${rowIndex}-${colIndex}`}
              role="gridcell"
              aria-label={`${squareLabel}${piece ? ", " + formatPieceName(piece) : ", empty"}${selectionHint}${focusHint}`}
              aria-selected={isSelected}
              aria-current={isFocused ? ("true" as const) : undefined}
              tabIndex={0}
              onFocus={() => setFocusedSquare(squareKey)}
              onBlur={() =>
                setFocusedSquare((prev) =>
                  prev === squareKey ? null : prev,
                )
              }
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleSquareClick(rowIndex, colIndex);
                }
              }}
              style={{
                backgroundColor: highlightedSquare
                  ? highlightedSquare.color
                  : isLight
                  ? colors.dark
                  : colors.light,
                width: "100%",
                height: "100%",
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                cursor: piece ? "grab" : "default",
                position: "relative",
                outline: "none",
                boxShadow: isSelected
                  ? selectedShadow
                  : isFocused
                  ? "inset 0 0 0 2px rgba(0, 200, 170, 0.7)"
                  : isLastMoveSquare
                  ? lastMoveShadow
                  : isHovered
                  ? "inset 0 0 0 2px rgba(0, 200, 170, 0.4)"
                  : "none",
                transition: "background-color 0.2s ease, box-shadow 0.1s ease",
              }}
              onContextMenu={(e) => handleRightClick(e, rowIndex, colIndex)}
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
    </div>
  );
};

export default React.memo(ChessboardComponent);