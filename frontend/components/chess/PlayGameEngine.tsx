"use client";

import React, { useState, useEffect, useCallback, useRef, useMemo } from "react";
import dynamic from "next/dynamic";
import { Chess } from "chess.js";
import { useParams, useRouter } from "next/navigation";
import { useChessSocket, projectClock } from "@/hook/useChessSocket";
import { FaUser, FaClock } from "react-icons/fa";
import { Web3StatusBar } from "@/components/Web3StatusBar";
import { useCheatDetection } from "@/hook/useCheatDetection";
import { GameResultOverlay } from "@/components/GameResultOverlay";
import type { GameResult, SeriesSummary } from "@/components/GameResultOverlay";
import {
  initialSeries,
  recordSeriesGame,
  seriesMultiplier,
  seriesOutcome,
  seriesPayout,
  type SeriesLength,
  type SeriesResult,
} from "@/lib/series";
import { CheatDetectionPanel } from "@/components/chess/CheatDetectionPanel";
import { useIsMobile } from "@/hook/use-mobile";
import { ErrorBoundary } from "@/components/ErrorBoundary";
import { useBoardAnnouncer } from "@/hook/useBoardAnnouncer";
import { KeyboardMoveInput } from "@/components/chess/KeyboardMoveInput";
import { VoiceMoveInput } from "@/components/chess/VoiceMoveInput";
import BlindfoldToggle from "@/components/chess/BlindfoldToggle";
import { useSpeechSynthesis } from "@/hook/useSpeechSynthesis";
import { useGamePreferences } from "@/context/GamePreferencesContext";
import AICompanionHUD from "@/components/chess/AICompanionHUD";
import { ReconnectionBanner } from "@/components/ReconnectionBanner";
import { ConnectionStatusIndicator } from "@/components/chess/ConnectionStatusIndicator";
import { useGameShortcuts } from "@/hook/useGameShortcuts";
import { ShortcutsHelpDialog } from "@/components/ShortcutsHelpDialog";
import { GameChatPanel } from "@/components/GameChatPanel";
import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet";

const ChessboardComponent = dynamic(
  () => import("@/components/chess/ChessboardComponent"),
  {
    ssr: false,
    loading: () => (
      <div className="aspect-square w-full max-w-[560px] rounded-md border-2 border-gray-700/50 p-1">
        <div className="grid grid-cols-8 grid-rows-8 gap-0 w-full h-full">
          {Array.from({ length: 64 }).map((_, i) => (
            <div
              key={i}
              className={`${
                (Math.floor(i / 8) + (i % 8)) % 2 === 0
                  ? "bg-gray-700/30"
                  : "bg-gray-600/20"
              } rounded-sm shimmer-bg`}
            />
          ))}
        </div>
      </div>
    ),
  },
);

const ChessboardWebGL3D = dynamic(
  () => import("@/components/chess/ChessboardWebGL3D").then((m) => m.ChessboardWebGL3D),
  { ssr: false },
);

type GameStatus =
  | "playing"
  | "checkmate"
  | "stalemate"
  | "draw"
  | "draw_agreed"
  | "aborted"
  | "resigned"
  | "timeout";

type OfferKind = "draw" | "takeback" | "rematch";
interface PendingOffer {
  kind: OfferKind;
  by: "self" | "opponent";
  /** For takebacks: ply count the board will be rolled back to. */
  ply?: number;
}

export default function PlayGameEngine({
  fen,
  initialMinutes = 10,
  playerColor = "white",
  seriesLength: seriesLengthProp = 1,
  stake = null,
  payout = null,
}: {
  fen?: string;
  /** Time control in minutes for each side (FE-116 / #1357). */
  initialMinutes?: number;
  /**
   * Colour assigned to the local player by matchmaking (FE-115). Defaults to
   * white for backwards compatibility, but the black player must pass "black"
   * so the board is oriented correctly and only their pieces can move.
   */
  playerColor?: "white" | "black";
  seriesLength?: SeriesLength;
  stake?: number | null;
  payout?: number | null;
}) {
  const seriesLength = seriesLengthProp;
  const params = useParams();
  const router = useRouter();
  const gameId = params.slug as string;

  const [game] = useState(new Chess(fen));
  const [position, setPosition] = useState(fen || "start");
  const [moveHistory, setMoveHistory] = useState<string[]>([]);
  const [whiteTime, setWhiteTime] = useState(() => Math.max(0, initialMinutes) * 60);
  const [blackTime, setBlackTime] = useState(() => Math.max(0, initialMinutes) * 60);
  const [gameStatus, setGameStatus] = useState<GameStatus>("playing");
  const [flaggedColor, setFlaggedColor] = useState<"white" | "black" | null>(null);
  const [resignation, setResignation] = useState<"self" | "opponent" | null>(null);
  const [isCheatPanelExpanded, setIsCheatPanelExpanded] = useState(false);
  const [isMoveHistoryOpen, setIsMoveHistoryOpen] = useState(false);
  const [isControlsOpen, setIsControlsOpen] = useState(false);
  const [boardOrientation, setBoardOrientation] = useState<"white" | "black">(
    playerColor,
  );
  const isMobile = useIsMobile();
  const { announcement, timeAnnouncement, announceMove, announceTimeAlert } =
    useBoardAnnouncer();
  const { preferences, setPreference } = useGamePreferences();
  const { speak: speakOpponentMove } = useSpeechSynthesis();

  const handleFlipBoard = useCallback(() => {
    setBoardOrientation((prev) => (prev === "white" ? "black" : "white"));
  }, []);

  const [pendingOffer, setPendingOffer] = useState<PendingOffer | null>(null);
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const [chatOpen, setChatOpen] = useState(false);
  const [resignConfirmOpen, setResignConfirmOpen] = useState(false);
  const [reviewPly, setReviewPly] = useState<number | null>(null);
  const chatInputRef = useRef<HTMLInputElement>(null);
  const handledActionIdRef = useRef(0);
  const [series, setSeries] = useState(() => initialSeries(seriesLength));
  const seriesRecordedRef = useRef(false);

  useEffect(() => {
    setSeries(initialSeries(seriesLength));
    seriesRecordedRef.current = false;
  }, [seriesLength]);

  const {
    status: socketStatus,
    sendMove,
    sendResign,
    sendGameAction,
    lastOpponentMove,
    opponentResigned,
    lastSyncedState,
    lastGameAction,
    serverClock,
    latencyMs,
    clockOffsetMs,
  } = useChessSocket(gameId);

  const checkGameStatus = useCallback(() => {
    if (game.isCheckmate()) {
      setGameStatus("checkmate");
    } else if (game.isStalemate()) {
      setGameStatus("stalemate");
    } else if (game.isDraw()) {
      setGameStatus("draw");
    }
  }, [game]);

  // BE-149: once the server sends clock snapshots, display server time projected
  // forward with latency correction. Until then fall back to a local countdown.
  useEffect(() => {
    if (!serverClock || gameStatus !== "playing") return;
    const tick = () => {
      const { whiteMs, blackMs } = projectClock(
        serverClock,
        performance.now(),
        Date.now(),
        latencyMs,
        clockOffsetMs,
      );
      setWhiteTime(Math.ceil(whiteMs / 1000));
      setBlackTime(Math.ceil(blackMs / 1000));
    };
    tick();
    const id = setInterval(tick, 200);
    return () => clearInterval(id);
  }, [serverClock, latencyMs, clockOffsetMs, gameStatus]);

  useEffect(() => {
    if (serverClock) return;
    if (socketStatus !== "connected" || gameStatus !== "playing") return;

    const id = setInterval(() => {
      const activeColor = game.turn();
      if (activeColor === "w") {
        setWhiteTime((t) => Math.max(0, t - 1));
      } else {
        setBlackTime((t) => Math.max(0, t - 1));
      }
    }, 1000);

    return () => clearInterval(id);
  }, [socketStatus, gameStatus, game, serverClock]);

  useEffect(() => {
    if (!lastOpponentMove || serverClock) return;
    const raw = lastOpponentMove as typeof lastOpponentMove & {
      whiteTime?: number;
      blackTime?: number;
    };
    if (typeof raw.whiteTime === "number") setWhiteTime(raw.whiteTime);
    if (typeof raw.blackTime === "number") setBlackTime(raw.blackTime);
  }, [lastOpponentMove, serverClock]);

  const {
    opponentAnalysis,
    playerAnalysis,
    recordMove: recordCheatMove,
    reset: resetCheatDetection,
    isActive: isCheatDetectionActive,
  } = useCheatDetection(playerColor);

  useEffect(() => {
    if (!lastOpponentMove) return;
    try {
      const fenBeforeOpponentMove = game.fen();
      const move = game.move({
        from: lastOpponentMove.from,
        to: lastOpponentMove.to,
        promotion: lastOpponentMove.promotion ?? "q",
      });
      if (move) {
        setPosition(game.fen());
        setMoveHistory((prev: string[]) => [...prev, move.san]);
        recordCheatMove(
          move.san,
          move,
          fenBeforeOpponentMove,
          playerColor === "white" ? "b" : "w",
          Math.ceil(game.moveNumber() / 2),
        );
        checkGameStatus();

        // Announce opponent move
        announceMove({
          color: playerColor === "white" ? "b" : "w",
          isCapture: move.captured != null,
          isCheck: game.isCheck(),
          piece: move.piece.toUpperCase(),
          from: move.from,
          to: move.to,
        });

        // FE-52: spoken announcement of opponent moves (configurable volume/rate)
        if (preferences.ttsOpponentMoves) {
          speakOpponentMove(`Opponent played ${move.san}`, {
            volume: preferences.ttsVolume,
            rate: preferences.ttsRate,
            lang: "en-US",
          });
        }
      }
    } catch {
      // illegal move from server — ignore
    }
  }, [
    lastOpponentMove,
    game,
    checkGameStatus,
    recordCheatMove,
    playerColor,
    announceMove,
    speakOpponentMove,
    preferences.ttsOpponentMoves,
    preferences.ttsVolume,
    preferences.ttsRate,
  ]);

  // FE-116 (#1357): a fallen flag ends the game — loss on time for that side.
  useEffect(() => {
    if (gameStatus !== "playing" || socketStatus !== "connected") return;
    if (whiteTime <= 0) {
      setFlaggedColor("white");
      setGameStatus("timeout");
    } else if (blackTime <= 0) {
      setFlaggedColor("black");
      setGameStatus("timeout");
    }
  }, [whiteTime, blackTime, gameStatus, socketStatus]);

  // FE-117 (#1358): the opponent resigned — we take the win.
  useEffect(() => {
    if (opponentResigned && gameStatus === "playing") {
      setResignation("opponent");
      setGameStatus("resigned");
    }
  }, [opponentResigned, gameStatus]);

  // FE-113: restore the board from the server's authoritative `sync` state after
  // a reconnect, applying any moves the opponent made while we were offline.
  useEffect(() => {
    if (!lastSyncedState) return;
    const { fen: syncedFen, moves, whiteTime: syncedWhite, blackTime: syncedBlack } =
      lastSyncedState;

    try {
      if (syncedFen) {
        game.load(syncedFen);
      } else if (moves.length > 0) {
        game.reset();
        for (const move of moves) {
          try {
            game.move({ from: move.from, to: move.to, promotion: move.promotion ?? "q" });
          } catch {
            break;
          }
        }
      }
      setPosition(game.fen());
      setMoveHistory(game.history());
      checkGameStatus();
    } catch {
      // Malformed FEN from the server — keep the local board rather than crash.
      return;
    }

    if (typeof syncedWhite === "number") setWhiteTime(syncedWhite);
    if (typeof syncedBlack === "number") setBlackTime(syncedBlack);
  }, [lastSyncedState, game, checkGameStatus]);

  const isMyTurn =
    socketStatus === "connected" &&
    ((playerColor === "white" && game.turn() === "w") ||
      (playerColor === "black" && game.turn() === "b"));

  const isReviewing = reviewPly !== null;

  useEffect(() => {
    setReviewPly(null);
  }, [moveHistory.length]);

  const displayPosition = useMemo(() => {
    if (reviewPly === null) return position;
    const replay = new Chess(fen);
    for (let i = 0; i < reviewPly; i++) {
      try {
        if (!replay.move(moveHistory[i])) break;
      } catch {
        break;
      }
    }
    return replay.fen();
  }, [fen, moveHistory, position, reviewPly]);

  const stepBack = useCallback(() => {
    setReviewPly((prev) =>
      prev === null ? Math.max(0, moveHistory.length - 1) : Math.max(0, prev - 1),
    );
  }, [moveHistory.length]);

  const stepForward = useCallback(() => {
    setReviewPly((prev) => {
      if (prev === null) return null;
      return prev >= moveHistory.length ? null : prev + 1;
    });
  }, [moveHistory.length]);

  const handleMove = useCallback(
    ({
      sourceSquare,
      targetSquare,
    }: {
      sourceSquare: string;
      targetSquare: string;
    }) => {
      if (!isMyTurn || isReviewing || gameStatus !== "playing") return false;

      try {
        const fenBeforeMyMove = game.fen();
        const move = game.move({
          from: sourceSquare,
          to: targetSquare,
          promotion: "q",
        });
        if (move === null) return false;

        requestAnimationFrame(() => setPosition(game.fen()));
        setMoveHistory((prev: string[]) => [...prev, move.san]);
        sendMove({
          from: sourceSquare,
          to: targetSquare,
          promotion: "q",
          ply: game.history().length,
        });
        recordCheatMove(
          move.san,
          move,
          fenBeforeMyMove,
          playerColor === "white" ? "w" : "b",
          Math.ceil(game.moveNumber() / 2),
        );
        checkGameStatus();

        // Announce player move
        announceMove({
          color: playerColor === "white" ? "w" : "b",
          isCapture: move.captured != null,
          isCheck: game.isCheck(),
          piece: move.piece.toUpperCase(),
          from: move.from,
          to: move.to,
        });
        return true;
      } catch {
        return false;
      }
    },
    [
      isMyTurn,
      isReviewing,
      game,
      gameStatus,
      sendMove,
      checkGameStatus,
      recordCheatMove,
      playerColor,
      announceMove,
    ],
  );

  const handleSanMove = useCallback(
    (san: string): boolean => {
      if (!isMyTurn || isReviewing || gameStatus !== "playing") return false;
      try {
        const fenBeforeMyMove = game.fen();
        const move = game.move(san);
        if (move === null) return false;

        requestAnimationFrame(() => setPosition(game.fen()));
        setMoveHistory((prev: string[]) => [...prev, move.san]);
        sendMove({
          from: move.from,
          to: move.to,
          promotion: move.promotion ?? "q",
          ply: game.history().length,
        });
        recordCheatMove(
          move.san,
          move,
          fenBeforeMyMove,
          playerColor === "white" ? "w" : "b",
          Math.ceil(game.moveNumber() / 2),
        );
        checkGameStatus();

        announceMove({
          color: playerColor === "white" ? "w" : "b",
          isCapture: move.captured != null,
          isCheck: game.isCheck(),
          piece: move.piece.toUpperCase(),
          from: move.from,
          to: move.to,
        });
        return true;
      } catch {
        return false;
      }
    },
    [
      isMyTurn,
      isReviewing,
      game,
      gameStatus,
      sendMove,
      checkGameStatus,
      recordCheatMove,
      playerColor,
      announceMove,
    ],
  );

  const timeAlertsRef = useRef({ gameId, announced: new Set<string>() });

  useEffect(() => {
    if (timeAlertsRef.current.gameId !== gameId) {
      timeAlertsRef.current = { gameId, announced: new Set<string>() };
    }
    if (gameStatus !== "playing") return;

    const myTime = playerColor === "white" ? whiteTime : blackTime;
    const colorName = playerColor === "white" ? "White" : "Black";
    const thresholds = [
      {
        key: `${gameId}:${playerColor}:30`,
        limit: 30,
        message: `30 seconds remaining for ${colorName}`,
      },
      {
        key: `${gameId}:${playerColor}:10`,
        limit: 10,
        message: `10 seconds remaining for ${colorName}`,
      },
      {
        key: `${gameId}:${playerColor}:0`,
        limit: 0,
        message: `Time is up for ${colorName}!`,
      },
    ];
    const next = thresholds.find(
      (threshold) =>
        myTime <= threshold.limit &&
        !timeAlertsRef.current.announced.has(threshold.key),
    );
    if (next) {
      timeAlertsRef.current.announced.add(next.key);
      announceTimeAlert(next.message);
    }
  }, [gameId, whiteTime, blackTime, playerColor, gameStatus, announceTimeAlert]);

  const handleResign = useCallback(() => {
    // FE-117 (#1358): tell the server (and therefore the opponent) so the game
    // completes and the opponent wins. The socket stays open so a rematch can
    // still be negotiated (BE-170).
    sendResign();
    setResignation("self");
    setGameStatus("resigned");
    setPendingOffer(null);
    resetCheatDetection();
  }, [sendResign, resetCheatDetection]);

  const requestResign = useCallback(() => {
    if (gameStatus === "playing") setResignConfirmOpen(true);
  }, [gameStatus]);

  const openChat = useCallback(() => {
    setChatOpen(true);
    requestAnimationFrame(() => chatInputRef.current?.focus());
  }, []);

  useEffect(() => {
    if (!resignConfirmOpen) return;
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setResignConfirmOpen(false);
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [resignConfirmOpen]);

  useGameShortcuts({
    flipBoard: handleFlipBoard,
    stepBack,
    stepForward,
    openChat,
    resign: requestResign,
    help: () => setHelpOpen((prev) => !prev),
  });

  // ── BE-170: draw / abort / takeback / rematch ─────────────────────────────
  const myColorCode = playerColor === "white" ? "w" : "b";
  const plyCount = moveHistory.length;
  const myMovesMade =
    playerColor === "white" ? Math.ceil(plyCount / 2) : Math.floor(plyCount / 2);
  const canAbort = gameStatus === "playing" && myMovesMade === 0;
  const canOfferDraw = gameStatus === "playing" && plyCount > 0 && !pendingOffer;
  const canTakeback = gameStatus === "playing" && myMovesMade > 0 && !pendingOffer;
  const canRematch = gameStatus !== "playing" && !pendingOffer;

  /** Ply count to roll back to so `requester`'s last move is undone. */
  const takebackTarget = useCallback(
    (requester: "w" | "b") => {
      const len = game.history().length;
      // If it's the requester's turn, the opponent already replied: undo two plies.
      return Math.max(0, game.turn() === requester ? len - 2 : len - 1);
    },
    [game],
  );

  const rollbackTo = useCallback(
    (targetPly: number) => {
      while (game.history().length > targetPly) {
        if (!game.undo()) break;
      }
      setPosition(game.fen());
      setMoveHistory(game.history());
      checkGameStatus();
    },
    [game, checkGameStatus],
  );

  const flashNotice = useCallback((msg: string) => {
    setActionNotice(msg);
  }, []);

  useEffect(() => {
    if (!actionNotice) return;
    const id = setTimeout(() => setActionNotice(null), 4000);
    return () => clearTimeout(id);
  }, [actionNotice]);

  const handleAbort = useCallback(() => {
    if (!canAbort) return;
    if (sendGameAction("abort")) {
      setPendingOffer(null);
      setGameStatus("aborted");
      resetCheatDetection();
    }
  }, [canAbort, sendGameAction, resetCheatDetection]);

  const handleOfferDraw = useCallback(() => {
    if (!canOfferDraw) return;
    if (sendGameAction("draw_offer")) {
      setPendingOffer({ kind: "draw", by: "self" });
      flashNotice("Draw offer sent.");
    }
  }, [canOfferDraw, sendGameAction, flashNotice]);

  const handleRequestTakeback = useCallback(() => {
    if (!canTakeback) return;
    const ply = takebackTarget(myColorCode);
    if (sendGameAction("takeback_offer", { ply })) {
      setPendingOffer({ kind: "takeback", by: "self", ply });
      flashNotice("Takeback requested.");
    }
  }, [canTakeback, takebackTarget, myColorCode, sendGameAction, flashNotice]);

  const handleOfferRematch = useCallback(() => {
    if (!canRematch) return;
    if (sendGameAction("rematch_offer")) {
      setPendingOffer({ kind: "rematch", by: "self" });
    }
  }, [canRematch, sendGameAction]);

  const handleCancelOffer = useCallback(() => {
    setPendingOffer(null);
  }, []);

  const respondToOffer = useCallback(
    (accept: boolean) => {
      if (!pendingOffer || pendingOffer.by !== "opponent") return;
      const { kind } = pendingOffer;
      if (kind === "draw") {
        sendGameAction(accept ? "draw_accept" : "draw_decline");
        if (accept) setGameStatus("draw_agreed");
      } else if (kind === "takeback") {
        const opponentCode = myColorCode === "w" ? "b" : "w";
        const ply = pendingOffer.ply ?? takebackTarget(opponentCode);
        sendGameAction(accept ? "takeback_accept" : "takeback_decline", { ply });
        if (accept) rollbackTo(ply);
      } else if (kind === "rematch") {
        sendGameAction(accept ? "rematch_accept" : "rematch_decline");
        if (accept) flashNotice("Rematch accepted — starting new game…");
      }
      setPendingOffer(null);
    },
    [pendingOffer, sendGameAction, myColorCode, takebackTarget, rollbackTo, flashNotice],
  );

  // Incoming negotiation events from the opponent / server.
  useEffect(() => {
    if (!lastGameAction || lastGameAction.id <= handledActionIdRef.current) return;
    handledActionIdRef.current = lastGameAction.id;
    const ev = lastGameAction;

    switch (ev.type) {
      case "draw_offer":
        if (gameStatus === "playing") setPendingOffer({ kind: "draw", by: "opponent" });
        break;
      case "draw_accept":
        setPendingOffer(null);
        if (gameStatus === "playing") setGameStatus("draw_agreed");
        break;
      case "draw_decline":
        setPendingOffer(null);
        flashNotice("Opponent declined the draw.");
        break;
      case "abort":
        setPendingOffer(null);
        if (gameStatus === "playing") setGameStatus("aborted");
        break;
      case "takeback_offer":
        if (gameStatus === "playing") {
          setPendingOffer({ kind: "takeback", by: "opponent", ply: ev.ply });
        }
        break;
      case "takeback_accept": {
        const ply =
          ev.ply ??
          (pendingOffer?.kind === "takeback" ? pendingOffer.ply : undefined) ??
          takebackTarget(myColorCode);
        setPendingOffer(null);
        rollbackTo(ply);
        flashNotice("Takeback accepted.");
        break;
      }
      case "takeback_decline":
        setPendingOffer(null);
        flashNotice("Opponent declined the takeback.");
        break;
      case "rematch_offer":
        if (gameStatus !== "playing") setPendingOffer({ kind: "rematch", by: "opponent" });
        break;
      case "rematch_accept":
        setPendingOffer(null);
        if (ev.newGameId) {
          const params = new URLSearchParams({ series: String(seriesLength) });
          if (stake !== null) params.set("stake", String(stake));
          if (payout !== null) params.set("payout", String(payout));
          router.push(`/play/${encodeURIComponent(ev.newGameId)}?${params.toString()}`);
        } else {
          flashNotice("Rematch accepted — waiting for the new game…");
        }
        break;
      case "rematch_decline":
        setPendingOffer(null);
        flashNotice("Opponent declined the rematch.");
        break;
      case "game_over":
        setPendingOffer(null);
        if (gameStatus === "playing") {
          setGameStatus(/abort/i.test(ev.reason ?? "") ? "aborted" : "draw_agreed");
        }
        break;
    }
  }, [
    lastGameAction,
    gameStatus,
    pendingOffer,
    takebackTarget,
    myColorCode,
    rollbackTo,
    flashNotice,
    router,
    seriesLength,
    stake,
    payout,
  ]);

  // A move by either side voids an outstanding draw offer from the mover.
  useEffect(() => {
    setPendingOffer((prev) => (prev && prev.kind === "draw" ? null : prev));
  }, [plyCount]);

  const formatTime = (seconds: number) => {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}:${s.toString().padStart(2, "0")}`;
  };

  const movePairs = moveHistory.reduce(
    (acc: string[][], move: string, i: number) => {
      if (i % 2 === 0) acc.push([move]);
      else acc[acc.length - 1].push(move);
      return acc;
    },
    [],
  );

  // FE-120 (#1361): persist the finished game's PGN and open the review view.
  const handleReview = useCallback(() => {
    try {
      sessionStorage.setItem(`knightverse:review:${gameId}`, game.pgn());
    } catch {
      // Storage can be unavailable (private mode) — the review page falls back
      // to a sample game.
    }
    router.push(`/game/${gameId}/review`);
  }, [game, gameId, router]);

  let overlayResult: GameResult | null = null;
  if (gameStatus === "checkmate") {
    overlayResult = game.turn() === "b" ? "white_wins" : "black_wins";
  } else if (gameStatus === "stalemate") {
    overlayResult = "stalemate";
  } else if (gameStatus === "draw") {
    overlayResult = "draw";
  } else if (gameStatus === "draw_agreed") {
    overlayResult = "draw_by_agreement";
  } else if (gameStatus === "aborted") {
    overlayResult = "aborted";
  } else if (gameStatus === "timeout") {
    overlayResult = flaggedColor === playerColor ? "loss_on_time" : "win_on_time";
  } else if (gameStatus === "resigned") {
    overlayResult =
      resignation === "self" ? "loss_by_resignation" : "win_by_resignation";
  }

  const gameSeriesResult: SeriesResult | null = (() => {
    if (!overlayResult || overlayResult === "aborted") return null;
    if (overlayResult === "white_wins") return playerColor === "white" ? "win" : "loss";
    if (overlayResult === "black_wins") return playerColor === "black" ? "win" : "loss";
    if (overlayResult === "loss_on_time" || overlayResult === "loss_by_resignation") return "loss";
    if (overlayResult === "win_on_time" || overlayResult === "win_by_resignation") return "win";
    return "draw";
  })();

  const displaySeries =
    gameSeriesResult && !seriesRecordedRef.current
      ? recordSeriesGame(series, gameSeriesResult)
      : series;
  const seriesFinalOutcome = seriesOutcome(displaySeries);
  const computedPayout = seriesPayout(stake, seriesLength, seriesFinalOutcome);
  const payoutLabel =
    seriesFinalOutcome === null
      ? null
      : seriesFinalOutcome === "win"
        ? payout !== null
          ? `${payout.toFixed(4)} XLM`
          : computedPayout !== null
            ? `${computedPayout.toFixed(4)} XLM`
            : `${seriesMultiplier(seriesLength)}× stake`
        : "No payout";

  const seriesSummary: SeriesSummary | undefined = overlayResult
    ? {
        label: `Bo${series.length}`,
        score: `${displaySeries.playerWins}–${displaySeries.opponentWins}${
          displaySeries.draws > 0 ? ` (${displaySeries.draws} drawn)` : ""
        }`,
        gameNumber: displaySeries.gameNumber,
        length: displaySeries.length,
        outcome: seriesFinalOutcome,
        payout: payoutLabel,
      }
    : undefined;

  useEffect(() => {
    if (!gameSeriesResult || seriesRecordedRef.current) return;
    seriesRecordedRef.current = true;
    setSeries((prev) => recordSeriesGame(prev, gameSeriesResult));
  }, [gameSeriesResult]);

  const handleNextGame = useCallback(() => {
    game.reset();
    setPosition(game.fen());
    setMoveHistory([]);
    const seconds = Math.max(0, initialMinutes) * 60;
    setWhiteTime(seconds);
    setBlackTime(seconds);
    setGameStatus("playing");
    setFlaggedColor(null);
    setResignation(null);
    setPendingOffer(null);
    setActionNotice(null);
    resetCheatDetection();
    seriesRecordedRef.current = false;
  }, [game, initialMinutes, resetCheatDetection]);

  const offerLabel: Record<OfferKind, string> = {
    draw: "a draw",
    takeback: "a takeback",
    rematch: "a rematch",
  };

  const incomingOffer =
    pendingOffer && pendingOffer.by === "opponent" ? (
      <div
        className="p-3 rounded-lg bg-blue-500/10 border border-blue-500/30"
        role="alertdialog"
        aria-live="assertive"
        aria-label={`Opponent offers ${offerLabel[pendingOffer.kind]}`}
      >
        <p className="text-sm text-blue-200 mb-2">
          Opponent offers {offerLabel[pendingOffer.kind]}.
        </p>
        <div className="flex gap-2">
          <button
            onClick={() => respondToOffer(true)}
            className="flex-1 py-2 rounded-lg bg-emerald-500/20 hover:bg-emerald-500/30 border border-emerald-500/40 text-emerald-300 text-sm font-medium"
          >
            Accept
          </button>
          <button
            onClick={() => respondToOffer(false)}
            className="flex-1 py-2 rounded-lg bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-red-300 text-sm font-medium"
          >
            Decline
          </button>
        </div>
      </div>
    ) : null;

  const outgoingOffer =
    pendingOffer && pendingOffer.by === "self" ? (
      <div className="flex items-center justify-between gap-2 p-2 rounded-lg bg-gray-900/60 border border-gray-700/50 text-xs text-gray-300">
        <span>Waiting for opponent to answer your {pendingOffer.kind} offer…</span>
        <button
          onClick={handleCancelOffer}
          className="text-gray-400 hover:text-white underline"
        >
          Dismiss
        </button>
      </div>
    ) : null;

  const actionButtonClass =
    "py-2.5 rounded-xl bg-gray-800/60 hover:bg-gray-700/60 border border-gray-700/50 text-gray-300 text-sm font-medium transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed";

  const gameActionsPanel = (
    <div
      className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4 space-y-3"
      role="group"
      aria-label="Game actions"
    >
      <h3 className="text-sm font-semibold text-gray-300">Actions</h3>
      {incomingOffer}
      {outgoingOffer}
      {actionNotice && (
        <p className="text-xs text-yellow-300" role="status">
          {actionNotice}
        </p>
      )}
      {gameStatus === "playing" ? (
        <div className="grid grid-cols-2 gap-2">
          {canAbort ? (
            <button onClick={handleAbort} className={actionButtonClass} aria-label="Abort game">
              Abort
            </button>
          ) : (
            <button
              onClick={handleOfferDraw}
              disabled={!canOfferDraw || socketStatus !== "connected"}
              className={actionButtonClass}
              aria-label="Offer draw"
            >
              Offer Draw
            </button>
          )}
          <button
            onClick={handleRequestTakeback}
            disabled={!canTakeback || socketStatus !== "connected"}
            className={actionButtonClass}
            aria-label="Request takeback"
          >
            Takeback
          </button>
        </div>
      ) : (
        <button
          onClick={handleOfferRematch}
          disabled={!canRematch || socketStatus !== "connected"}
          className={`w-full ${actionButtonClass}`}
          aria-label="Offer rematch"
        >
          Offer Rematch
        </button>
      )}
    </div>
  );

  const gameStatusPanel = (
    <div className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4 animate-fade-in">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold text-gray-300">Game Status</h3>
        <ConnectionStatusIndicator
          status={socketStatus}
          className="hidden lg:flex"
        />
      </div>

      {gameStatus !== "playing" && (
        <div
          className="p-3 rounded-lg bg-gradient-to-r from-yellow-500/10 to-orange-500/10 border border-yellow-500/30 mb-3 animate-scale-in"
          role="alert"
          aria-live="assertive"
        >
          <p className="text-sm font-bold text-yellow-400">
            {gameStatus === "checkmate" && "Checkmate!"}
            {gameStatus === "stalemate" && "Stalemate!"}
            {gameStatus === "draw" && "Draw!"}
            {gameStatus === "draw_agreed" && "Draw agreed."}
            {gameStatus === "aborted" && "Game aborted."}
            {gameStatus === "timeout" &&
              (flaggedColor === playerColor
                ? "Time's up — you lost on time."
                : "Opponent flagged — you win on time!")}
            {gameStatus === "resigned" &&
              (resignation === "self"
                ? "Resigned!"
                : "Opponent resigned — you win!")}
          </p>
        </div>
      )}

      {game.isCheck() && gameStatus === "playing" && (
        <div
          className="p-2 rounded-lg bg-red-500/10 border border-red-500/30 mb-3 animate-scale-in"
          role="alert"
          aria-live="assertive"
        >
          <p className="text-sm font-bold text-red-400">Check!</p>
        </div>
      )}

      <div className="text-xs text-gray-500 flex justify-between gap-2">
        <span>Game ID: {gameId?.slice(0, 12)}...</span>
        {serverClock && (
          <span title="Clock is server-authoritative, corrected for latency">
            Server clock · {latencyMs}ms
          </span>
        )}
      </div>
    </div>
  );

  const moveHistoryPanel = (
    <div
      className="rounded-xl border border-gray-700/50 bg-gray-800/40 p-4"
      role="region"
      aria-label="Move history"
    >
      <button
        className="flex items-center justify-between w-full text-left"
        onClick={() => isMobile && setIsMoveHistoryOpen((prev) => !prev)}
        aria-expanded={isMobile ? isMoveHistoryOpen : true}
      >
        <h3 className="text-sm font-semibold text-gray-300">Moves</h3>
        <span className="text-gray-500 text-xs lg:hidden">
          {isMoveHistoryOpen ? "▲" : "▼"}
        </span>
      </button>
      <div
        className={`max-h-64 overflow-y-auto space-y-0.5 mt-3 ${isMobile && !isMoveHistoryOpen ? "hidden" : ""}`}
      >
        {movePairs.length === 0 ? (
          <p className="text-xs text-gray-500 italic">
            No moves yet.{" "}
            {isMyTurn ? "Your turn to move!" : "Waiting for opponent..."}
          </p>
        ) : (
          movePairs.map((pair, i) => (
            <div
              key={i}
              className="flex items-center gap-2 py-1 px-2 rounded hover:bg-gray-700/30 text-sm"
            >
              <span className="text-gray-500 w-6 text-right text-xs">
                {i + 1}.
              </span>
              <span className="text-white font-mono text-xs w-16">
                {pair[0]}
              </span>
              <span className="text-gray-400 font-mono text-xs w-16">
                {pair[1] ?? ""}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );

  const boardControlButtons = (
    <div className="grid grid-cols-2 gap-2">
      <button
        onClick={handleFlipBoard}
        aria-label="Flip board orientation"
        className="py-2.5 rounded-xl bg-gray-800/60 hover:bg-gray-700/60 border border-gray-700/50 text-gray-300 text-sm font-medium transition-all duration-300"
      >
        Flip Board
      </button>
      <button
        onClick={requestResign}
        disabled={gameStatus !== "playing"}
        aria-label="Resign game"
        className="py-2.5 rounded-xl bg-red-500/10 hover:bg-red-500/20 border border-red-500/30 text-red-400 text-sm font-medium transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Resign
      </button>
    </div>
  );

  const renderMoveEntryControls = (idPrefix: string, enableGlobalShortcut: boolean) => (
    <div className="space-y-3">
      <VoiceMoveInput
        onSubmitMove={handleSanMove}
        isGameActive={gameStatus === "playing"}
        isMyTurn={isMyTurn}
        enabled={preferences.voiceMoveEnabled}
      />
      <KeyboardMoveInput
        onSubmitMove={handleSanMove}
        isGameActive={gameStatus === "playing"}
        isMyTurn={isMyTurn}
        suggestions={game.moves()}
        idPrefix={idPrefix}
        enableGlobalShortcut={enableGlobalShortcut}
      />
    </div>
  );

  const rematchControls = (
    <div className="space-y-2">
      {incomingOffer}
      {outgoingOffer}
      {actionNotice && (
        <p className="text-xs text-yellow-300" role="status">
          {actionNotice}
        </p>
      )}
      {!pendingOffer && (
        <button
          onClick={handleOfferRematch}
          disabled={!canRematch || socketStatus !== "connected"}
          className="w-full flex items-center justify-center gap-2 px-5 py-3 bg-gradient-to-r from-amber-500 to-orange-600 hover:from-amber-400 hover:to-orange-500 text-white font-semibold rounded-xl transition-all duration-300 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          Rematch
        </button>
      )}
    </div>
  );

  return (
    <div
      className="min-h-screen p-2 sm:p-4 md:p-8"
      role="region"
      aria-label="Online Chess Game"
    >
      <div className="max-w-7xl mx-auto">
        <div className="flex items-center justify-between mb-4">
          <button
            onClick={() => router.push("/")}
            aria-label="Back to lobby"
            className="text-gray-400 hover:text-white transition-colors text-sm flex items-center gap-2"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              className="h-4 w-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M15 19l-7-7 7-7"
              />
            </svg>
            Back to Lobby
          </button>
          <Web3StatusBar />
        </div>

        <ReconnectionBanner
          isDisconnected={socketStatus === "reconnecting" || socketStatus === "disconnected"}
        />

        <div className="flex flex-col lg:flex-row gap-6 items-start justify-center">
          <div
            className="mx-auto w-full max-w-[600px] min-w-0 lg:mx-0"
            role="region"
            aria-label="Chess board"
          >
            <div
              className="sticky top-0 z-20 -mx-2 mb-3 flex items-center justify-between border-b border-gray-800/60 bg-gray-950/95 px-2 py-2 backdrop-blur sm:mx-0 sm:px-1"
              role="status"
              aria-label="Opponent info"
            >
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-red-400 to-red-600 flex items-center justify-center">
                  <FaUser className="text-white text-xs" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-white">Opponent</p>
                  <p className="text-xs text-gray-400">
                    {playerColor === "white" ? "Black" : "White"}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/60 border border-gray-700/30">
                <FaClock className="text-gray-400 text-xs" />
                <span className="font-mono text-sm text-gray-200">
                  {formatTime(playerColor === "white" ? blackTime : whiteTime)}
                </span>
              </div>
            </div>

            <div className="w-full min-w-0">
              <AICompanionHUD
                evaluation={null}
                isVictory={gameStatus === "checkmate" && game.turn() === "w"}
                isTactical={game.isCheck()}
                commentary={
                  game.isCheck()
                    ? "Stay alert, the king is under pressure."
                    : moveHistory.length
                      ? "I am tracking the position and looking for the next idea."
                      : "Let's find a strong opening move."
                }
                className="mb-3"
              />
              <ErrorBoundary componentName="Chessboard">
                {preferences.boardRenderer === "3d" ? (
                  <ChessboardWebGL3D
                    fen={displayPosition === "start" ? game.fen() : displayPosition}
                    interactive
                    onMove={(from, to) =>
                      handleMove({ sourceSquare: from, targetSquare: to })
                    }
                  />
                ) : (
                  <ChessboardComponent
                    position={displayPosition}
                    onDrop={handleMove}
                    orientation={boardOrientation}
                    blindfoldMode={preferences.blindfoldMode}
                    isMyTurn={isMyTurn && !isReviewing}
                    premovesEnabled={!isReviewing && socketStatus === "connected"}
                  />
                )}
              </ErrorBoundary>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-3">
              <BlindfoldToggle
                blindfoldMode={preferences.blindfoldMode}
                onToggle={(v) => setPreference("blindfoldMode", v)}
              />
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-2">
              <button
                type="button"
                onClick={stepBack}
                aria-label="Step one move back"
                className="rounded-lg bg-gray-800/60 px-3 py-2 text-sm text-gray-300 hover:bg-gray-700/60"
              >
                ‹
              </button>
              <button
                type="button"
                onClick={stepForward}
                aria-label="Step one move forward"
                className="rounded-lg bg-gray-800/60 px-3 py-2 text-sm text-gray-300 hover:bg-gray-700/60"
              >
                ›
              </button>
              {isReviewing && (
                <button
                  type="button"
                  onClick={() => setReviewPly(null)}
                  className="rounded-lg bg-blue-600/20 px-3 py-2 text-xs text-blue-300 hover:bg-blue-600/30"
                >
                  Back to live
                </button>
              )}
              <button
                type="button"
                onClick={openChat}
                aria-label="Open game chat"
                className="rounded-lg bg-gray-800/60 px-3 py-2 text-sm text-gray-300 hover:bg-gray-700/60"
              >
                Chat
              </button>
              <button
                type="button"
                onClick={() => setHelpOpen(true)}
                aria-label="Keyboard shortcuts help"
                className="rounded-lg bg-gray-800/60 px-3 py-2 text-sm text-gray-300 hover:bg-gray-700/60"
              >
                ?
              </button>
            </div>

            <div
              className="sticky bottom-0 z-20 -mx-2 mt-3 flex items-center justify-between border-t border-gray-800/60 bg-gray-950/95 px-2 py-2 backdrop-blur sm:mx-0 sm:px-1"
              role="status"
              aria-label="Your info"
            >
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-gradient-to-br from-teal-400 to-blue-600 flex items-center justify-center">
                  <FaUser className="text-white text-xs" />
                </div>
                <div>
                  <p className="text-sm font-semibold text-white">You</p>
                  <p className="text-xs text-gray-400 capitalize">
                    {playerColor}
                    {isMyTurn && (
                      <span className="ml-2 text-emerald-400">(Your turn)</span>
                    )}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-800/60 border border-gray-700/30">
                <FaClock className="text-gray-400 text-xs" />
                <span className="font-mono text-sm text-gray-200">
                  {formatTime(playerColor === "white" ? whiteTime : blackTime)}
                </span>
              </div>
            </div>

            <div className="mt-3 hidden space-y-3 lg:block">
              {renderMoveEntryControls("desktop-keyboard-move", true)}
            </div>

            <div className="mt-3 flex items-center gap-2 lg:hidden">
              <Sheet open={isControlsOpen} onOpenChange={setIsControlsOpen}>
                <SheetTrigger asChild>
                  <button
                    type="button"
                    aria-label="Open game controls"
                    className="flex-1 py-2.5 rounded-xl bg-gray-800/60 hover:bg-gray-700/60 border border-gray-700/50 text-gray-300 text-sm font-medium transition-all duration-300"
                  >
                    Game Controls
                  </button>
                </SheetTrigger>
                <SheetContent
                  side="bottom"
                  className="max-h-[85vh] overflow-y-auto border-gray-700 bg-gray-900 px-0 pb-6"
                >
                  <SheetHeader className="px-4">
                    <SheetTitle className="text-white">Game Controls</SheetTitle>
                  </SheetHeader>
                  <div className="space-y-3 px-4">
                    {renderMoveEntryControls("mobile-keyboard-move", false)}
                    {gameStatusPanel}
                    {gameActionsPanel}
                    {moveHistoryPanel}
                    <CheatDetectionPanel
                      opponentAnalysis={opponentAnalysis}
                      playerAnalysis={playerAnalysis}
                      isActive={isCheatDetectionActive}
                      isExpanded={isCheatPanelExpanded}
                      onToggle={() =>
                        setIsCheatPanelExpanded((prev: boolean) => !prev)
                      }
                    />
                    {boardControlButtons}
                    <ConnectionStatusIndicator
                      status={socketStatus}
                      className="justify-center"
                    />
                  </div>
                </SheetContent>
              </Sheet>
              <ConnectionStatusIndicator
                status={socketStatus}
                className="shrink-0"
              />
            </div>
          </div>

          <div
            className="hidden w-full space-y-4 lg:block lg:w-80"
            role="complementary"
            aria-label="Game controls"
          >
            {gameStatusPanel}

            <div
              className="rounded-xl border border-teal-500/30 bg-teal-500/10 p-4"
              role="status"
              aria-label="Series score"
            >
              <div className="flex items-center justify-between gap-2">
                <h3 className="text-sm font-semibold text-teal-300">
                  Series · Bo{series.length}
                </h3>
                <span className="text-xs text-gray-400">
                  {seriesFinalOutcome
                    ? "Final"
                    : `Game ${displaySeries.gameNumber} of ${displaySeries.length}`}
                </span>
              </div>
              <p className="mt-3 text-lg font-bold text-white">
                You {displaySeries.playerWins} — {displaySeries.opponentWins} Opponent
              </p>
              {displaySeries.draws > 0 && (
                <p className="mt-1 text-xs text-gray-400">
                  {displaySeries.draws} drawn
                </p>
              )}
              {seriesFinalOutcome && (
                <p className="mt-2 text-sm font-semibold text-emerald-300">
                  {seriesFinalOutcome === "win"
                    ? "Series won"
                    : seriesFinalOutcome === "loss"
                      ? "Series lost"
                      : "Series drawn"}
                  {payoutLabel ? ` · Payout ${payoutLabel}` : ""}
                </p>
              )}
              {!seriesFinalOutcome && overlayResult && (
                <p className="mt-2 text-xs text-teal-200">
                  Next game: Game {displaySeries.gameNumber} of {displaySeries.length}
                </p>
              )}
            </div>

            {gameActionsPanel}
            {moveHistoryPanel}
            <CheatDetectionPanel
              opponentAnalysis={opponentAnalysis}
              playerAnalysis={playerAnalysis}
              isActive={isCheatDetectionActive}
              isExpanded={isCheatPanelExpanded}
              onToggle={() => setIsCheatPanelExpanded((prev: boolean) => !prev)}
            />
            {boardControlButtons}
          </div>
        </div>
      </div>

      <div
        role="status"
        aria-live="polite"
        aria-atomic="true"
        aria-label="Chess move announcements"
        className="sr-only"
      >
        {announcement}
      </div>
      <div
        role="alert"
        aria-live="assertive"
        aria-atomic="true"
        aria-label="Chess time alerts"
        className="sr-only"
      >
        {timeAnnouncement}
      </div>

      {overlayResult && (
        <GameResultOverlay
          result={overlayResult}
          onPlayAgain={() => {
            if (seriesSummary && !seriesFinalOutcome) handleNextGame();
            else router.push("/");
          }}
          onPlayOnline={() => router.push("/")}
          onReview={handleReview}
          extraActions={rematchControls}
          series={seriesSummary}
        />
      )}

      <ShortcutsHelpDialog
        open={helpOpen}
        onClose={() => setHelpOpen(false)}
      />
      <GameChatPanel
        ref={chatInputRef}
        open={chatOpen}
        onClose={() => setChatOpen(false)}
      />
      {resignConfirmOpen && (
        <div
          className="fixed inset-0 z-[130] flex items-center justify-center bg-black/70 p-4"
          role="dialog"
          aria-modal="true"
          aria-labelledby="resign-confirm-title"
        >
          <div className="w-full max-w-sm rounded-xl border border-red-500/40 bg-gray-900 p-5 text-white shadow-2xl">
            <h2 id="resign-confirm-title" className="text-lg font-bold text-red-300">
              Resign this game?
            </h2>
            <p className="mt-2 text-sm text-gray-400">
              Your opponent will be declared the winner.
            </p>
            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setResignConfirmOpen(false)}
                className="rounded-lg bg-gray-800 px-4 py-2 text-sm hover:bg-gray-700"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => {
                  setResignConfirmOpen(false);
                  handleResign();
                }}
                className="rounded-lg bg-red-600 px-4 py-2 text-sm font-semibold hover:bg-red-500"
              >
                Resign
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
