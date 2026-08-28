"""Tactical pattern extraction for natural-language blunder coaching.

This module turns raw engine output (a played move plus the principal variation
that refutes it) into structured tactical motifs -- Hanging Piece, Fork, Pin,
Skewer and Back-Rank Mate -- that the natural language layer can verbalise.

Everything is derived from the actual board with ``python-chess``: motifs are
only reported when the pieces, squares and moves involved really exist and the
moves are legal. Nothing here invents a move or a threat.
"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from enum import Enum
from typing import Optional, Sequence

import chess

__all__ = [
    "TacticalMotif",
    "MotifDetection",
    "TacticalAnalysis",
    "TacticalPatternExtractor",
    "PIECE_VALUES",
    "parse_move",
    "move_label",
    "piece_name",
    "color_name",
]


# Material values in pawns; the king is given a sentinel value so that it always
# sorts first when we describe the victims of a tactic.
PIECE_VALUES: dict[int, int] = {
    chess.PAWN: 1,
    chess.KNIGHT: 3,
    chess.BISHOP: 3,
    chess.ROOK: 5,
    chess.QUEEN: 9,
    chess.KING: 100,
}

_PIECE_NAMES: dict[int, str] = {
    chess.PAWN: "Pawn",
    chess.KNIGHT: "Knight",
    chess.BISHOP: "Bishop",
    chess.ROOK: "Rook",
    chess.QUEEN: "Queen",
    chess.KING: "King",
}

# Ray directions as (file_delta, rank_delta) for each sliding piece type.
_DIAGONALS = ((1, 1), (1, -1), (-1, 1), (-1, -1))
_ORTHOGONALS = ((1, 0), (-1, 0), (0, 1), (0, -1))
_SLIDER_DIRECTIONS: dict[int, tuple[tuple[int, int], ...]] = {
    chess.BISHOP: _DIAGONALS,
    chess.ROOK: _ORTHOGONALS,
    chess.QUEEN: _DIAGONALS + _ORTHOGONALS,
}

# Severity assigned to a forced mate so it always outranks material motifs.
MATE_SEVERITY = 1000


class TacticalMotif(str, Enum):
    """Tactical patterns the extractor can recognise."""

    HANGING_PIECE = "hanging_piece"
    FORK = "fork"
    PIN = "pin"
    SKEWER = "skewer"
    BACK_RANK_MATE = "back_rank_mate"
    MATE_THREAT = "mate_threat"
    MATERIAL_LOSS = "material_loss"


# Ordering used when a position contains several threats: mate first, then the
# motifs that win the most material.
_THREAT_PRIORITY: dict[TacticalMotif, int] = {
    TacticalMotif.BACK_RANK_MATE: 0,
    TacticalMotif.MATE_THREAT: 1,
    TacticalMotif.FORK: 2,
    TacticalMotif.SKEWER: 3,
    TacticalMotif.PIN: 4,
    TacticalMotif.MATERIAL_LOSS: 5,
    TacticalMotif.HANGING_PIECE: 6,
}


def piece_name(piece_type: int) -> str:
    """Return the capitalised English name of a piece type."""
    return _PIECE_NAMES.get(piece_type, "piece")


def color_name(color: chess.Color) -> str:
    """Return ``"White"`` or ``"Black"``."""
    return "White" if color == chess.WHITE else "Black"


def parse_move(board: chess.Board, token: Optional[str]) -> Optional[chess.Move]:
    """Parse a UCI or SAN move token, returning ``None`` unless it is legal.

    Args:
        board: Position the move is played from.
        token: Move in UCI (``"g1f3"``) or SAN (``"Nf3"``) notation.

    Returns:
        The legal :class:`chess.Move`, or ``None`` if the token is malformed,
        illegal, or not a move at all. Callers rely on this to avoid ever
        repeating a move that cannot be played.
    """
    if not token or not isinstance(token, str):
        return None

    candidate = token.strip()
    if not candidate:
        return None

    move: Optional[chess.Move] = None
    try:
        move = board.parse_uci(candidate)
    except ValueError:
        try:
            move = board.parse_san(candidate)
        except ValueError:
            return None

    return move if move in board.legal_moves else None


def move_label(board: chess.Board, move: chess.Move) -> str:
    """Format a move the way a coach writes it, e.g. ``"35.Ne7+"`` or ``"34...Rd8"``.

    Args:
        board: Position *before* the move is played.
        move: A legal move in that position.
    """
    san = board.san(move)
    if board.turn == chess.WHITE:
        return f"{board.fullmove_number}.{san}"
    return f"{board.fullmove_number}...{san}"


@dataclass(frozen=True)
class MotifDetection:
    """A single tactical motif found on the board."""

    motif: TacticalMotif
    attacker: Optional[str] = None
    attacker_square: Optional[str] = None
    victims: tuple[str, ...] = ()
    victim_squares: tuple[str, ...] = ()
    severity: int = 0
    #: True when the hanging piece is the very piece the blunder moved.
    self_inflicted: bool = False
    #: True when the piece has at least one defender but is still losable.
    defended: bool = False
    #: True when the refutation itself completes the motif, as opposed to only
    #: starting a forced sequence or threatening it for the move after.
    immediate: bool = True
    #: True when the mate is played out inside the validated engine line, as
    #: opposed to merely being threatened once the refutation lands.
    forced: bool = False

    @property
    def name(self) -> str:
        """Human readable motif name, e.g. ``"Back-Rank Mate"``."""
        return _MOTIF_TITLES[self.motif]

    def victim_phrase(self) -> str:
        """Describe the victims, e.g. ``"your King and Rook"``."""
        if not self.victims:
            return "your position"
        if len(self.victims) == 1:
            return f"your {self.victims[0]}"
        if self.victims[0] == self.victims[1]:
            return f"both your {self.victims[0]}s"
        return "your " + " and ".join(self.victims[:2])


_MOTIF_TITLES: dict[TacticalMotif, str] = {
    TacticalMotif.HANGING_PIECE: "Hanging Piece",
    TacticalMotif.FORK: "Fork",
    TacticalMotif.PIN: "Pin",
    TacticalMotif.SKEWER: "Skewer",
    TacticalMotif.BACK_RANK_MATE: "Back-Rank Mate",
    TacticalMotif.MATE_THREAT: "Mate Threat",
    TacticalMotif.MATERIAL_LOSS: "Material Loss",
}


@dataclass
class TacticalAnalysis:
    """Structured result of analysing a blunder."""

    fen: str
    hero: chess.Color = chess.WHITE
    villain: chess.Color = chess.BLACK
    valid: bool = False
    blunder_label: str = ""
    best_label: str = ""
    refutation_label: str = ""
    #: Motif describing what the played move gave away (hanging piece, weak back rank).
    cause: Optional[MotifDetection] = None
    #: Motif describing how the opponent punishes it (fork, pin, skewer, mate).
    threat: Optional[MotifDetection] = None
    motifs: list[MotifDetection] = field(default_factory=list)
    is_mate: bool = False
    #: Material the opponent wins outright in the analysed line, in pawns.
    material_swing: int = 0

    @property
    def motif_names(self) -> list[str]:
        """Unique titles of the motifs found, most important first."""
        names: list[str] = []
        for motif in self.motifs:
            if motif.name not in names:
                names.append(motif.name)
        return names


class TacticalPatternExtractor:
    """Derives tactical motifs from a position, a blunder and the refuting line.

    The extractor is stateless and cheap: a full extraction runs a handful of
    ``python-chess`` move generations, well inside the coaching latency budget.
    """

    def extract(
        self,
        fen: str,
        blunder_move: str,
        best_move: str = "",
        engine_pv: Optional[Sequence[str]] = None,
    ) -> TacticalAnalysis:
        """Analyse a blunder and return the motifs it allows.

        Args:
            fen: Position *before* the blunder was played.
            blunder_move: The move actually played, in UCI or SAN.
            best_move: The move the engine preferred, in UCI or SAN.
            engine_pv: The engine's principal variation refuting the blunder,
                starting with the opponent's reply. A line that starts with the
                blunder itself is accepted too.

        Returns:
            A :class:`TacticalAnalysis`. ``valid`` is ``False`` when the FEN or
            the blunder move could not be verified, in which case no motifs are
            reported rather than guessed.
        """
        try:
            board = chess.Board(fen)
        except ValueError:
            return TacticalAnalysis(fen=fen, valid=False)

        if not board.is_valid():
            return TacticalAnalysis(fen=fen, valid=False)

        hero = board.turn
        villain = not hero
        analysis = TacticalAnalysis(fen=fen, hero=hero, villain=villain)

        blunder = parse_move(board, blunder_move)
        if blunder is None:
            return analysis

        analysis.valid = True
        analysis.blunder_label = move_label(board, blunder)

        best = parse_move(board, best_move)
        if best is not None and best != blunder:
            analysis.best_label = move_label(board, best)

        after = board.copy()
        after.push(blunder)

        pv_moves = self._parse_pv(board, after, blunder, engine_pv)
        refutation = pv_moves[0] if pv_moves else None
        if refutation is not None:
            analysis.refutation_label = move_label(after, refutation)

        motifs: list[MotifDetection] = []

        cause = self._detect_cause(board, after, blunder, refutation)
        if cause is not None:
            motifs.append(cause)
        analysis.cause = cause

        threat = self._detect_threat(board, after, pv_moves, hero, villain)
        if threat is not None:
            motifs.append(threat)
        analysis.threat = threat

        analysis.is_mate = threat is not None and threat.motif in (
            TacticalMotif.BACK_RANK_MATE,
            TacticalMotif.MATE_THREAT,
        )
        analysis.material_swing = self._material_swing(after, pv_moves, hero)
        analysis.motifs = sorted(motifs, key=lambda m: _THREAT_PRIORITY[m.motif])
        return analysis

    # ------------------------------------------------------------------
    # Principal variation handling
    # ------------------------------------------------------------------

    def _parse_pv(
        self,
        board: chess.Board,
        after: chess.Board,
        blunder: chess.Move,
        engine_pv: Optional[Sequence[str]],
    ) -> list[chess.Move]:
        """Validate the PV move by move, stopping at the first illegal token."""
        tokens = list(engine_pv or [])
        if tokens and parse_move(board, tokens[0]) == blunder:
            # Some engines report the line including the move under review.
            tokens = tokens[1:]

        line: list[chess.Move] = []
        cursor = after.copy()
        for token in tokens:
            move = parse_move(cursor, token)
            if move is None:
                break
            line.append(move)
            cursor.push(move)
        return line

    # ------------------------------------------------------------------
    # Cause: what the blunder gave away
    # ------------------------------------------------------------------

    def _detect_cause(
        self,
        before: chess.Board,
        after: chess.Board,
        blunder: chess.Move,
        refutation: Optional[chess.Move],
    ) -> Optional[MotifDetection]:
        """Find what the played move loosened: a hanging piece or the back rank."""
        hero = before.turn
        loose_before = self._loose_pieces(before, hero)
        loose_after = self._loose_pieces(after, hero)

        # Only blame the move for pieces it actually loosened.
        stale = {(sq, det.victims) for sq, det in loose_before.items()}
        fresh = {
            square: det
            for square, det in loose_after.items()
            if (square, det.victims) not in stale
        }
        candidates = fresh or loose_after

        if candidates:
            captured = (
                refutation.to_square
                if refutation is not None and after.is_capture(refutation)
                else None
            )
            if captured is not None and captured in candidates:
                chosen = candidates[captured]
            else:
                chosen = max(candidates.values(), key=lambda det: det.severity)
            if chosen.victim_squares and chosen.victim_squares[0] == chess.square_name(
                blunder.to_square
            ):
                chosen = replace(chosen, self_inflicted=True)
            return chosen

        if self._back_rank_abandoned(before, after, blunder, hero):
            king_square = after.king(hero)
            return MotifDetection(
                motif=TacticalMotif.BACK_RANK_MATE,
                victims=("King",),
                victim_squares=(chess.square_name(king_square),)
                if king_square is not None
                else (),
                severity=MATE_SEVERITY,
            )
        return None

    def _back_rank_abandoned(
        self,
        before: chess.Board,
        after: chess.Board,
        blunder: chess.Move,
        hero: chess.Color,
    ) -> bool:
        """True when the move left the king's back rank indefensible.

        Either the move boxed the king in itself, or it marched the last heavy
        piece off a back rank the king cannot escape from.
        """
        if not self._back_rank_is_weak(after, hero):
            return False

        if not self._back_rank_is_weak(before, hero):
            return True

        back_rank = 0 if hero == chess.WHITE else 7
        mover = after.piece_at(blunder.to_square)
        left_back_rank = (
            mover is not None
            and mover.piece_type in (chess.ROOK, chess.QUEEN)
            and chess.square_rank(blunder.from_square) == back_rank
            and chess.square_rank(blunder.to_square) != back_rank
        )
        if not left_back_rank:
            return False

        # Only a real abandonment if no other heavy piece still guards the rank.
        guards = after.pieces(chess.ROOK, hero) | after.pieces(chess.QUEEN, hero)
        return not any(chess.square_rank(square) == back_rank for square in guards)

    def _loose_pieces(
        self, board: chess.Board, owner: chess.Color
    ) -> dict[int, MotifDetection]:
        """Map squares of ``owner``'s attacked, insufficiently defended pieces."""
        villain = not owner
        loose: dict[int, MotifDetection] = {}

        for square, piece in board.piece_map().items():
            if piece.color != owner or piece.piece_type == chess.KING:
                continue

            attackers = board.attackers(villain, square)
            if not attackers:
                continue

            cheapest = min(
                PIECE_VALUES[board.piece_type_at(sq)] for sq in attackers
            )
            defenders = board.attackers(owner, square)
            value = PIECE_VALUES[piece.piece_type]
            if defenders and cheapest >= value:
                continue

            loose[square] = MotifDetection(
                motif=TacticalMotif.HANGING_PIECE,
                victims=(piece_name(piece.piece_type),),
                victim_squares=(chess.square_name(square),),
                severity=value if not defenders else max(value - cheapest, 1),
                defended=bool(defenders),
            )
        return loose

    def _back_rank_is_weak(self, board: chess.Board, owner: chess.Color) -> bool:
        """True when ``owner``'s king sits on its back rank with no escape square."""
        king_square = board.king(owner)
        if king_square is None:
            return False

        back_rank = 0 if owner == chess.WHITE else 7
        if chess.square_rank(king_square) != back_rank:
            return False

        step = 1 if owner == chess.WHITE else -1
        king_file = chess.square_file(king_square)
        escapes = 0
        for file_delta in (-1, 0, 1):
            file = king_file + file_delta
            if not 0 <= file <= 7:
                continue
            square = chess.square(file, back_rank + step)
            occupant = board.piece_at(square)
            if occupant is None or occupant.color != owner:
                escapes += 1
        if escapes:
            return False

        # A weak back rank only matters if the opponent owns a heavy piece.
        heavy = board.pieces(chess.ROOK, not owner) | board.pieces(
            chess.QUEEN, not owner
        )
        return bool(heavy)

    # ------------------------------------------------------------------
    # Threat: how the opponent punishes it
    # ------------------------------------------------------------------

    def _detect_threat(
        self,
        before: chess.Board,
        after: chess.Board,
        pv_moves: list[chess.Move],
        hero: chess.Color,
        villain: chess.Color,
    ) -> Optional[MotifDetection]:
        """Classify the tactic the refutation creates."""
        if not pv_moves:
            return None

        refutation = pv_moves[0]
        refuted = after.copy()
        refuted.push(refutation)

        mate = self._detect_mate(after, pv_moves, hero)
        if mate is not None:
            return mate

        candidates: list[MotifDetection] = []

        fork = self._detect_fork(refuted, refutation.to_square, hero, villain)
        if fork is not None:
            candidates.append(fork)

        candidates.extend(self._detect_line_tactics(before, refuted, hero, villain))

        if after.is_capture(refutation):
            captured = after.piece_at(refutation.to_square)
            if captured is not None:
                candidates.append(
                    MotifDetection(
                        motif=TacticalMotif.MATERIAL_LOSS,
                        attacker=piece_name(after.piece_type_at(refutation.from_square)),
                        attacker_square=chess.square_name(refutation.to_square),
                        victims=(piece_name(captured.piece_type),),
                        victim_squares=(chess.square_name(refutation.to_square),),
                        severity=PIECE_VALUES[captured.piece_type],
                    )
                )

        if not candidates:
            return None

        # Mate is handled above, so what is left competes on material at stake;
        # motif priority only breaks ties.
        return sorted(
            candidates,
            key=lambda m: (-m.severity, _THREAT_PRIORITY[m.motif]),
        )[0]

    def _detect_mate(
        self,
        after: chess.Board,
        pv_moves: list[chess.Move],
        hero: chess.Color,
    ) -> Optional[MotifDetection]:
        """Detect mate delivered in the PV, or mate threatened right after it."""
        cursor = after.copy()
        for index, move in enumerate(pv_moves):
            mating_piece = cursor.piece_at(move.from_square)
            cursor.push(move)
            if cursor.is_checkmate() and cursor.turn == hero:
                return self._classify_mate(
                    cursor,
                    move.to_square,
                    mating_piece,
                    hero,
                    immediate=index == 0,
                    forced=True,
                )

        refuted = after.copy()
        refuted.push(pv_moves[0])
        threatened = self._mate_in_one(refuted)
        if threatened is None:
            return None

        move, board_after_mate = threatened
        mating_piece = refuted.piece_at(move.from_square)
        return self._classify_mate(
            board_after_mate,
            move.to_square,
            mating_piece,
            hero,
            immediate=False,
            forced=False,
        )

    def _mate_in_one(
        self, board: chess.Board
    ) -> Optional[tuple[chess.Move, chess.Board]]:
        """Return the opponent's mate-in-one if they were allowed to move now."""
        if board.is_check() or board.is_game_over():
            return None

        probe = board.copy()
        probe.push(chess.Move.null())
        for move in probe.legal_moves:
            probe.push(move)
            if probe.is_checkmate():
                mated = probe.copy()
                probe.pop()
                return move, mated
            probe.pop()
        return None

    def _classify_mate(
        self,
        mated_board: chess.Board,
        mating_square: int,
        mating_piece: Optional[chess.Piece],
        hero: chess.Color,
        immediate: bool,
        forced: bool,
    ) -> MotifDetection:
        """Label a mate as back-rank when it is delivered along the king's back rank."""
        king_square = mated_board.king(hero)
        back_rank = 0 if hero == chess.WHITE else 7
        is_back_rank = (
            king_square is not None
            and chess.square_rank(king_square) == back_rank
            and chess.square_rank(mating_square) == back_rank
            and mating_piece is not None
            and mating_piece.piece_type in (chess.ROOK, chess.QUEEN)
        )
        return MotifDetection(
            motif=TacticalMotif.BACK_RANK_MATE
            if is_back_rank
            else TacticalMotif.MATE_THREAT,
            attacker=piece_name(mating_piece.piece_type) if mating_piece else None,
            attacker_square=chess.square_name(mating_square),
            victims=("King",),
            victim_squares=(chess.square_name(king_square),)
            if king_square is not None
            else (),
            severity=MATE_SEVERITY,
            immediate=immediate,
            forced=forced,
        )

    def _detect_fork(
        self,
        board: chess.Board,
        square: int,
        hero: chess.Color,
        villain: chess.Color,
    ) -> Optional[MotifDetection]:
        """Detect a double attack by the piece that just landed on ``square``."""
        forker = board.piece_at(square)
        if forker is None or forker.color != villain:
            return None

        forker_value = PIECE_VALUES[forker.piece_type]
        targets: list[tuple[int, chess.Piece]] = []
        for target_square in board.attacks(square):
            target = board.piece_at(target_square)
            if target is None or target.color != hero:
                continue
            value = PIECE_VALUES[target.piece_type]
            defended = bool(board.attackers(hero, target_square))
            if target.piece_type == chess.KING or value > forker_value or not defended:
                targets.append((target_square, target))

        if len(targets) < 2:
            return None

        targets.sort(key=lambda item: PIECE_VALUES[item[1].piece_type], reverse=True)
        # A real fork must win something: the king, or a piece worth more than
        # the forker (or an undefended piece alongside another target).
        best_value = PIECE_VALUES[targets[0][1].piece_type]
        if best_value <= forker_value and targets[0][1].piece_type != chess.KING:
            return None

        second_value = PIECE_VALUES[targets[1][1].piece_type]
        severity = second_value if targets[0][1].piece_type == chess.KING else best_value
        return MotifDetection(
            motif=TacticalMotif.FORK,
            attacker=piece_name(forker.piece_type),
            attacker_square=chess.square_name(square),
            victims=tuple(piece_name(p.piece_type) for _, p in targets[:2]),
            victim_squares=tuple(chess.square_name(sq) for sq, _ in targets[:2]),
            severity=severity,
        )

    def _detect_line_tactics(
        self,
        before_refutation: chess.Board,
        after_refutation: chess.Board,
        hero: chess.Color,
        villain: chess.Color,
    ) -> list[MotifDetection]:
        """Detect pins and skewers created by the refutation (direct or discovered)."""
        existing = self._line_relations(before_refutation, villain, hero)
        created = self._line_relations(after_refutation, villain, hero)
        return [
            detection
            for key, detection in created.items()
            if key not in existing
        ]

    def _line_relations(
        self,
        board: chess.Board,
        attacker_color: chess.Color,
        victim_color: chess.Color,
    ) -> dict[tuple[int, int, int], MotifDetection]:
        """Map every pin/skewer alignment ``attacker -> front -> back`` on the board."""
        relations: dict[tuple[int, int, int], MotifDetection] = {}

        for piece_type, directions in _SLIDER_DIRECTIONS.items():
            for origin in board.pieces(piece_type, attacker_color):
                origin_file = chess.square_file(origin)
                origin_rank = chess.square_rank(origin)
                for file_delta, rank_delta in directions:
                    found: list[tuple[int, chess.Piece]] = []
                    file, rank = origin_file + file_delta, origin_rank + rank_delta
                    while 0 <= file <= 7 and 0 <= rank <= 7 and len(found) < 2:
                        square = chess.square(file, rank)
                        occupant = board.piece_at(square)
                        if occupant is not None:
                            if occupant.color != victim_color:
                                break
                            found.append((square, occupant))
                        file += file_delta
                        rank += rank_delta

                    if len(found) < 2:
                        continue

                    (front_square, front), (back_square, back) = found
                    front_value = PIECE_VALUES[front.piece_type]
                    back_value = PIECE_VALUES[back.piece_type]
                    if front_value == back_value:
                        continue

                    is_pin = back_value > front_value
                    relations[(origin, front_square, back_square)] = MotifDetection(
                        motif=TacticalMotif.PIN if is_pin else TacticalMotif.SKEWER,
                        attacker=piece_name(piece_type),
                        attacker_square=chess.square_name(origin),
                        victims=(
                            piece_name(front.piece_type),
                            piece_name(back.piece_type),
                        ),
                        victim_squares=(
                            chess.square_name(front_square),
                            chess.square_name(back_square),
                        ),
                        severity=min(front_value, back_value),
                    )
        return relations

    # ------------------------------------------------------------------
    # Material
    # ------------------------------------------------------------------

    def _material_swing(
        self,
        after: chess.Board,
        pv_moves: list[chess.Move],
        hero: chess.Color,
    ) -> int:
        """Net material (in pawns) the opponent wins over the validated PV."""
        cursor = after.copy()
        swing = 0
        for move in pv_moves:
            if cursor.is_capture(move):
                if cursor.is_en_passant(move):
                    victim_color: chess.Color | None = not cursor.turn
                    value = PIECE_VALUES[chess.PAWN]
                else:
                    captured = cursor.piece_at(move.to_square)
                    victim_color = captured.color if captured else None
                    value = PIECE_VALUES[captured.piece_type] if captured else 0
                if victim_color == hero:
                    swing += value
                elif victim_color is not None:
                    swing -= value
            cursor.push(move)
        return swing
