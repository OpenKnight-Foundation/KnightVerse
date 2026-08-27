"""Tests for the conversational blunder coach.

The suite runs 20 tactical benchmark positions through
:func:`gpu_worker.nl_agent.explain_blunder` and checks that every explanation is
short, tactically correct, and free of invented moves or threats.
"""

import asyncio
import re
import time
import unittest
from unittest.mock import AsyncMock, MagicMock

import chess

from gpu_worker.models import AnalysisResult, PersonalityTraits
from gpu_worker.nl_agent import (
    COACHING_DISABLED_MESSAGE,
    MAX_EXPLANATION_LENGTH,
    UNVERIFIABLE_MESSAGE,
    BlunderCoach,
    NaturalLanguageAgent,
    explain_blunder,
)
from gpu_worker.nl_models import ComplexityLevel, IntentType
from gpu_worker.tactics import TacticalPatternExtractor, parse_move

#: Latency budget for a single explanation, in milliseconds.
LATENCY_BUDGET_MS = 350


class Benchmark:
    """One tactical benchmark position and the motif it must produce."""

    def __init__(
        self,
        name: str,
        fen: str,
        blunder: str,
        best: str,
        pv: list[str],
        motif: str,
        phrases: tuple[str, ...] = (),
    ) -> None:
        self.name = name
        self.fen = fen
        self.blunder = blunder
        self.best = best
        self.pv = pv
        self.motif = motif
        self.phrases = phrases

    def explain(self, **kwargs) -> str:
        """Run the coach on this position."""
        return explain_blunder(self.fen, self.blunder, self.best, self.pv, **kwargs)


# 20 benchmark positions covering the five motifs the coach must recognise.
BENCHMARKS: list[Benchmark] = [
    # -- Forks ---------------------------------------------------------------
    Benchmark(
        "knight forks King and Rook",
        "2rb2k1/5ppp/8/3N4/8/8/P5P1/R5K1 b - - 0 34",
        "d8a5", "d8e7", ["d5e7"],
        "Fork", ("fork your King and Rook", "35.Ne7+"),
    ),
    Benchmark(
        "knight forks King and Queen",
        "6k1/5p1p/2q5/8/4N3/8/P5P1/R5K1 b - - 0 34",
        "c6d7", "c6c5", ["e4f6"],
        "Fork", ("fork your King and Queen", "35.Nf6+"),
    ),
    Benchmark(
        "pawn forks Knight and Bishop",
        "6k1/5ppp/2n5/5b2/3P4/8/P5P1/R5K1 b - - 0 34",
        "f5e6", "f5g6", ["d4d5"],
        "Fork", ("fork your Knight and Bishop", "35.d5"),
    ),
    Benchmark(
        "queen forks King and Rook",
        "r5k1/6pp/8/8/8/8/P5P1/R2Q2K1 b - - 0 34",
        "a8a5", "a8e8", ["d1d5"],
        "Fork", ("fork your King and Rook", "35.Qd5+"),
    ),
    Benchmark(
        "knight forks both Rooks",
        "r3r3/5pkp/6p1/1N6/8/8/P5P1/R5K1 b - - 0 34",
        "a8c8", "e8e2", ["b5d6"],
        "Fork", ("fork both your Rooks", "35.Nd6"),
    ),
    # -- Pins ----------------------------------------------------------------
    Benchmark(
        "bishop pins Knight to Queen",
        "3q2k1/5pp1/5n1p/8/8/8/P5P1/R1B3K1 b - - 0 34",
        "h6h5", "d8e7", ["c1g5"],
        "Pin", ("pin your Knight on f6 to your Queen", "35.Bg5"),
    ),
    Benchmark(
        "rook pins Bishop to King",
        "4k3/p6p/3b4/8/8/8/P5P1/R5K1 b - - 0 34",
        "d6e5", "e8f8", ["a1e1"],
        "Pin", ("pin your Bishop on e5 to your King", "35.Re1"),
    ),
    Benchmark(
        "bishop pins Knight to Rook",
        "r7/5pkp/6p1/8/5B2/2n5/P5P1/4R1K1 b - - 0 34",
        "a8a5", "c3d5", ["f4d2"],
        "Pin", ("pin your Knight on c3 to your Rook", "35.Bd2"),
    ),
    Benchmark(
        "discovered pin on the e-file",
        "4k3/p6p/8/4b3/4N3/8/P5P1/4R1K1 b - - 0 34",
        "a7a6", "e5c3", ["e4d6"],
        "Pin", ("pin your Bishop on e5 to your King", "35.Nd6+"),
    ),
    # -- Skewers -------------------------------------------------------------
    Benchmark(
        "bishop skewers King and Rook",
        "r7/5pkp/8/8/8/8/P5P1/R1B3K1 b - - 0 34",
        "a8h8", "a8a2", ["c1b2"],
        "Skewer", ("skewer your King and win the Rook on h8", "35.Bb2+"),
    ),
    Benchmark(
        "rook skewers Queen and Rook",
        "R7/5pkp/6p1/4q3/8/8/3r1PPP/6K1 b - - 0 34",
        "e5d4", "e5e2", ["a8d8"],
        "Skewer", ("skewer your Queen and win the Rook on d2", "35.Rd8"),
    ),
    Benchmark(
        "bishop skewers Queen and Knight",
        "6k1/n1q2p1p/6p1/8/8/B7/5PP1/R5K1 b - - 0 34",
        "c7b6", "c7c1", ["a3c5"],
        "Skewer", ("skewer your Queen and win the Knight on a7", "35.Bc5"),
    ),
    # -- Back-rank mates -----------------------------------------------------
    Benchmark(
        "back-rank mate forced in three",
        "4r1k1/5ppp/8/8/8/8/P5P1/3R2K1 b - - 0 34",
        "e8e6", "e8d8", ["d1d8", "e6e8", "d8e8"],
        "Back-Rank Mate", ("abandons your back rank", "force back-rank mate", "35.Rd8+"),
    ),
    Benchmark(
        "back-rank mate after the rook leaves",
        "2r3k1/5ppp/8/8/8/8/P5P1/4R1K1 b - - 0 34",
        "c8c5", "c8d8", ["e1e8", "c5c8", "e8c8"],
        "Back-Rank Mate", ("abandons your back rank", "back-rank mate", "35.Re8#"),
    ),
    Benchmark(
        "back-rank mate in one by the queen",
        "6k1/1b3ppp/8/8/8/8/P5P1/R2Q2K1 b - - 0 34",
        "b7a6", "b7e4", ["d1d8"],
        "Back-Rank Mate", ("back-rank mate", "35.Qd8#"),
    ),
    # -- Hanging pieces ------------------------------------------------------
    Benchmark(
        "knight walks onto an attacked square",
        "6k1/5ppp/2n5/8/8/6B1/P5P1/R5K1 b - - 0 34",
        "c6e5", "c6d4", ["g3e5"],
        "Hanging Piece", ("your Knight on e5 en prise", "win your Knight", "35.Bxe5"),
    ),
    Benchmark(
        "bishop abandons the knight it defended",
        "6k1/5ppp/4b3/3n4/8/8/P5P1/3R2K1 b - - 0 34",
        "e6h3", "d5f6", ["d1d5"],
        "Hanging Piece", ("leaves your Knight on d5 undefended", "35.Rxd5"),
    ),
    Benchmark(
        "queen steps onto a defended diagonal",
        "3q2k1/5ppp/8/8/8/7B/P5P1/R5K1 b - - 0 34",
        "d8d7", "d8e7", ["h3d7"],
        "Hanging Piece", ("your Queen on d7 en prise", "win your Queen", "35.Bxd7"),
    ),
    Benchmark(
        "pawn push loosens the rook",
        "6k1/5ppp/4p3/3r4/2B5/8/P5P1/R5K1 b - - 0 34",
        "e6e5", "d5d8", ["c4d5"],
        "Hanging Piece", ("leaves your Rook on d5 undefended", "win your Rook", "35.Bxd5"),
    ),
    Benchmark(
        "White hangs a bishop to a pawn",
        "r5k1/5ppp/4p3/8/2B5/8/P4PPP/R5K1 w - - 0 30",
        "c4d5", "c4b3", ["e6d5"],
        "Hanging Piece", ("your Bishop on d5 en prise", "30...exd5"),
    ),
]

#: Move labels the coach emits, e.g. ``35.Ne7+`` or ``34...Rxa2``.
MOVE_LABEL = re.compile(r"\b\d+\.(?:\.\.)?([^\s,.]+)")

#: Piece-on-square claims, e.g. ``your Knight on f6``.
PIECE_CLAIM = re.compile(r"your (King|Queen|Rook|Bishop|Knight|Pawn) on ([a-h][1-8])")

PIECE_TYPES = {
    "King": chess.KING,
    "Queen": chess.QUEEN,
    "Rook": chess.ROOK,
    "Bishop": chess.BISHOP,
    "Knight": chess.KNIGHT,
    "Pawn": chess.PAWN,
}


def line_boards(benchmark: Benchmark) -> list[chess.Board]:
    """Return every position along the benchmark's verified line."""
    board = chess.Board(benchmark.fen)
    boards = [board.copy()]
    blunder = parse_move(board, benchmark.blunder)
    if blunder is None:
        return boards
    board.push(blunder)
    boards.append(board.copy())
    for token in benchmark.pv:
        move = parse_move(board, token)
        if move is None:
            break
        board.push(move)
        boards.append(board.copy())
    return boards


def mate_in_one(board: chess.Board) -> bool:
    """True when the side that just moved could mate on their next turn."""
    if board.is_game_over() or board.is_check():
        return False
    probe = board.copy()
    probe.push(chess.Move.null())
    for move in probe.legal_moves:
        probe.push(move)
        mate = probe.is_checkmate()
        probe.pop()
        if mate:
            return True
    return False


class TestBenchmarkPositions(unittest.TestCase):
    """Explanations must be accurate across the 20 benchmark positions."""

    def test_suite_covers_every_motif(self):
        """The benchmark set exercises all five required motifs."""
        self.assertEqual(len(BENCHMARKS), 20)
        motifs = {benchmark.motif for benchmark in BENCHMARKS}
        self.assertEqual(
            motifs,
            {"Fork", "Pin", "Skewer", "Back-Rank Mate", "Hanging Piece"},
        )

    def test_expected_motif_is_identified(self):
        """The extractor names the motif each position was built around."""
        extractor = TacticalPatternExtractor()
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                analysis = extractor.extract(
                    benchmark.fen, benchmark.blunder, benchmark.best, benchmark.pv
                )
                self.assertTrue(analysis.valid)
                self.assertIn(benchmark.motif, analysis.motif_names)

    def test_explanations_use_expected_chess_terminology(self):
        """Each explanation names the tactic, the squares and the refutation."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                text = benchmark.explain()
                for phrase in benchmark.phrases:
                    self.assertIn(phrase, text)

    def test_explanations_stay_under_the_length_cap(self):
        """Responses stay readable at a glance during play."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                self.assertLessEqual(len(benchmark.explain()), MAX_EXPLANATION_LENGTH)

    def test_explanations_start_with_the_played_move(self):
        """The coach opens by naming the move that was actually played."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                board = chess.Board(benchmark.fen)
                played = parse_move(board, benchmark.blunder)
                self.assertIsNotNone(played)
                self.assertIn(board.san(played), benchmark.explain())

    def test_only_legal_moves_are_mentioned(self):
        """Every move label in the text is legal somewhere in the verified line."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                text = benchmark.explain()
                boards = line_boards(benchmark)
                for san in MOVE_LABEL.findall(text):
                    legal_somewhere = False
                    for board in boards:
                        try:
                            board.parse_san(san)
                        except ValueError:
                            continue
                        legal_somewhere = True
                        break
                    self.assertTrue(
                        legal_somewhere, f"{san!r} is not legal in {benchmark.name}"
                    )

    def test_named_pieces_are_really_on_the_named_squares(self):
        """Every "your Piece on square" claim matches the actual board."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                text = benchmark.explain()
                boards = line_boards(benchmark)
                hero = chess.Board(benchmark.fen).turn
                for piece_name, square_name in PIECE_CLAIM.findall(text):
                    expected = chess.Piece(PIECE_TYPES[piece_name], hero)
                    square = chess.parse_square(square_name)
                    self.assertTrue(
                        any(board.piece_at(square) == expected for board in boards),
                        f"no {piece_name} on {square_name} in {benchmark.name}",
                    )

    def test_mate_is_only_claimed_when_mate_exists(self):
        """A mention of mate is always backed by a mate on the board."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                text = benchmark.explain()
                if "mate" not in text:
                    continue
                boards = line_boards(benchmark)
                self.assertTrue(
                    any(board.is_checkmate() for board in boards)
                    or any(mate_in_one(board) for board in boards[1:]),
                    f"{benchmark.name} claims mate with none available",
                )

    def test_latency_is_within_budget(self):
        """A single explanation stays well under the 350ms play-time budget."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                start = time.perf_counter()
                benchmark.explain()
                elapsed_ms = (time.perf_counter() - start) * 1000
                self.assertLess(elapsed_ms, LATENCY_BUDGET_MS)


class TestGuardrails(unittest.TestCase):
    """The coach refuses to invent moves, threats or advice."""

    def setUp(self):
        self.benchmark = BENCHMARKS[0]

    def test_illegal_blunder_move_is_refused(self):
        """An unplayable move yields the safe fallback, not a guess."""
        text = explain_blunder(self.benchmark.fen, "a1a8", "d8e7", ["d5e7"])
        self.assertEqual(text, UNVERIFIABLE_MESSAGE)

    def test_malformed_move_is_refused(self):
        """Garbage input is rejected rather than parsed loosely."""
        text = explain_blunder(self.benchmark.fen, "not-a-move", "d8e7", ["d5e7"])
        self.assertEqual(text, UNVERIFIABLE_MESSAGE)

    def test_invalid_fen_is_refused(self):
        """A position that cannot be parsed produces no tactical claims."""
        text = explain_blunder("not a fen", "d8a5", "d8e7", ["d5e7"])
        self.assertEqual(text, UNVERIFIABLE_MESSAGE)

    def test_illegal_refutation_is_ignored(self):
        """An illegal engine line is dropped instead of being quoted back."""
        text = explain_blunder(self.benchmark.fen, "d8a5", "d8e7", ["h8h1"])
        self.assertNotIn("Rh1", text)
        self.assertNotIn("fork", text)

    def test_line_is_truncated_at_the_first_illegal_move(self):
        """Moves after an illegal token never reach the explanation."""
        analysis = TacticalPatternExtractor().extract(
            "4r1k1/5ppp/8/8/8/8/P5P1/3R2K1 b - - 0 34",
            "e8e6",
            "e8d8",
            ["d1d8", "a1a8", "d8e8"],
        )
        self.assertFalse(analysis.is_mate)

    def test_pv_may_repeat_the_blunder_move(self):
        """Engines that echo the move under review are handled."""
        with_echo = explain_blunder(
            self.benchmark.fen, "d8a5", "d8e7", ["d8a5", "d5e7"]
        )
        self.assertEqual(with_echo, self.benchmark.explain())

    def test_illegal_moves_never_appear_across_the_benchmarks(self):
        """No benchmark explanation mentions a move outside the verified line."""
        for benchmark in BENCHMARKS:
            with self.subTest(benchmark.name):
                text = benchmark.explain()
                self.assertNotIn("??", text)
                self.assertNotIn("None", text)

    def test_san_and_uci_inputs_agree(self):
        """The same blunder in SAN or UCI produces the same coaching line."""
        for benchmark in BENCHMARKS[:5]:
            with self.subTest(benchmark.name):
                board = chess.Board(benchmark.fen)
                san = board.san(parse_move(board, benchmark.blunder))
                self.assertEqual(
                    benchmark.explain(),
                    explain_blunder(benchmark.fen, san, benchmark.best, benchmark.pv),
                )


class TestCompanionMode(unittest.TestCase):
    """Move advice is gated on companion mode during rated play."""

    def setUp(self):
        self.benchmark = BENCHMARKS[0]

    def test_rated_game_without_companion_mode_withholds_everything(self):
        """No move is suggested in a rated game the player has not opted into."""
        text = self.benchmark.explain(companion_mode=False, rated_game=True)
        self.assertEqual(text, COACHING_DISABLED_MESSAGE)
        self.assertEqual(MOVE_LABEL.findall(text), [])

    def test_rated_game_with_companion_mode_still_coaches(self):
        """Companion mode re-enables coaching during rated play."""
        text = self.benchmark.explain(companion_mode=True, rated_game=True)
        self.assertIn("35.Ne7+", text)

    def test_companion_mode_off_omits_the_suggested_improvement(self):
        """Casual play without the companion explains but does not prescribe."""
        text = self.benchmark.explain(companion_mode=False)
        self.assertNotIn("Better was", text)
        self.assertIn("fork your King and Rook", text)

    def test_companion_mode_on_suggests_the_engine_move(self):
        """With the companion enabled the better move is offered."""
        self.assertIn("Better was 34...Be7", self.benchmark.explain())


class TestPersonality(unittest.TestCase):
    """The coaching line adopts the companion's configured tone."""

    def test_each_tone_has_its_own_voice(self):
        benchmark = BENCHMARKS[0]
        prefixes = {
            "neutral": "34...",
            "aggressive": "Ouch!",
            "humorous": "Yikes!",
            "formal": "Note:",
        }
        for tone, prefix in prefixes.items():
            with self.subTest(tone):
                text = benchmark.explain(traits=PersonalityTraits(tone=tone))
                self.assertTrue(text.startswith(prefix))
                self.assertLessEqual(len(text), MAX_EXPLANATION_LENGTH)

    def test_unknown_tone_falls_back_to_neutral(self):
        benchmark = BENCHMARKS[0]
        text = benchmark.explain(traits=PersonalityTraits(tone="mysterious"))
        self.assertEqual(text, benchmark.explain())

    def test_tone_does_not_change_the_tactical_content(self):
        benchmark = BENCHMARKS[0]
        text = benchmark.explain(traits=PersonalityTraits(tone="humorous"))
        self.assertIn("fork your King and Rook with 35.Ne7+", text)


class TestBlunderCoach(unittest.TestCase):
    """Structured output of the coach."""

    def setUp(self):
        self.coach = BlunderCoach()
        self.benchmark = BENCHMARKS[0]

    def explain(self, **kwargs):
        return self.coach.explain(
            fen=self.benchmark.fen,
            blunder_move=self.benchmark.blunder,
            best_move=self.benchmark.best,
            engine_pv=self.benchmark.pv,
            **kwargs,
        )

    def test_explanation_carries_its_evidence(self):
        explanation = self.explain()
        self.assertEqual(explanation.motifs, ["Fork"])
        self.assertEqual(explanation.blunder_move, "34...Ba5")
        self.assertEqual(explanation.best_move, "34...Be7")
        self.assertEqual(explanation.refutation, "35.Ne7+")
        self.assertFalse(explanation.is_mate)
        self.assertGreater(explanation.latency_ms, 0)

    def test_material_swing_is_measured_over_the_line(self):
        benchmark = BENCHMARKS[17]  # queen steps onto a defended diagonal
        explanation = self.coach.explain(
            fen=benchmark.fen,
            blunder_move=benchmark.blunder,
            best_move=benchmark.best,
            engine_pv=benchmark.pv,
        )
        self.assertEqual(explanation.material_swing, 9)

    def test_mate_is_flagged(self):
        benchmark = BENCHMARKS[14]  # back-rank mate in one by the queen
        explanation = self.coach.explain(
            fen=benchmark.fen,
            blunder_move=benchmark.blunder,
            best_move=benchmark.best,
            engine_pv=benchmark.pv,
        )
        self.assertTrue(explanation.is_mate)

    def test_custom_length_budget_is_respected(self):
        explanation = self.explain(max_length=60)
        self.assertLessEqual(len(explanation.text), 60)

    def test_to_dict_is_serialisable(self):
        payload = self.explain().to_dict()
        self.assertEqual(payload["refutation"], "35.Ne7+")
        self.assertEqual(payload["motifs"], ["Fork"])


class TestAgentPipeline(unittest.TestCase):
    """Blunder detection wired to the natural language generator."""

    def setUp(self):
        self.benchmark = BENCHMARKS[0]
        self.pool = MagicMock()
        self.agent = NaturalLanguageAgent(self.pool)

    def set_evaluations(self, before: float, after: float):
        """Queue the two engine analyses coach_move performs."""
        self.pool.submit = AsyncMock(
            side_effect=[
                AnalysisResult(
                    request_id="before",
                    best_move=self.benchmark.best,
                    evaluation=before,
                    depth=16,
                    principal_variation=[self.benchmark.best],
                ),
                AnalysisResult(
                    request_id="after",
                    best_move=self.benchmark.pv[0],
                    evaluation=after,
                    depth=16,
                    principal_variation=self.benchmark.pv,
                ),
            ]
        )

    def test_evaluation_collapse_is_explained(self):
        """A move that loses the evaluation is coached, not just scored."""
        self.set_evaluations(before=0.2, after=4.5)
        response = asyncio.run(
            self.agent.coach_move(self.benchmark.fen, self.benchmark.blunder)
        )
        self.assertTrue(response.metadata["is_blunder"])
        self.assertIn("fork your King and Rook", response.natural_language_response)
        self.assertEqual(response.metadata["motifs"], ["Fork"])
        self.assertAlmostEqual(response.metadata["eval_loss"], 4.7)
        self.assertEqual(response.intent, IntentType.EXPLAIN_MOVE)

    def test_sound_move_is_not_coached(self):
        """A move that holds the evaluation is reported as fine."""
        self.set_evaluations(before=0.2, after=-0.1)
        response = asyncio.run(
            self.agent.coach_move(self.benchmark.fen, self.benchmark.blunder)
        )
        self.assertFalse(response.metadata["is_blunder"])
        self.assertNotIn("fork", response.natural_language_response)

    def test_engine_best_move_is_not_treated_as_a_blunder(self):
        """Playing the engine's own choice is never called a mistake."""
        self.set_evaluations(before=0.2, after=4.5)
        response = asyncio.run(
            self.agent.coach_move(self.benchmark.fen, self.benchmark.best)
        )
        self.assertFalse(response.metadata["is_blunder"])

    def test_unverifiable_move_short_circuits_the_engine(self):
        """An illegal move is refused before any analysis is requested."""
        self.pool.submit = AsyncMock()
        response = asyncio.run(self.agent.coach_move(self.benchmark.fen, "a1a8"))
        self.assertEqual(response.natural_language_response, UNVERIFIABLE_MESSAGE)
        self.pool.submit.assert_not_called()

    def test_rated_game_without_companion_mode_is_refused(self):
        """No analysis and no move suggestion during unopted rated play."""
        self.pool.submit = AsyncMock()
        response = asyncio.run(
            self.agent.coach_move(
                self.benchmark.fen,
                self.benchmark.blunder,
                companion_mode=False,
                rated_game=True,
            )
        )
        self.assertEqual(response.natural_language_response, COACHING_DISABLED_MESSAGE)
        self.assertIsNone(response.best_move)
        self.pool.submit.assert_not_called()

    def test_companion_mode_off_hides_the_best_move_field(self):
        """Casual play without the companion still withholds the engine move."""
        self.set_evaluations(before=0.2, after=4.5)
        response = asyncio.run(
            self.agent.coach_move(
                self.benchmark.fen, self.benchmark.blunder, companion_mode=False
            )
        )
        self.assertIsNone(response.best_move)

    def test_personality_reaches_the_generated_text(self):
        """The companion's tone is applied through the pipeline."""
        self.set_evaluations(before=0.2, after=4.5)
        response = asyncio.run(
            self.agent.coach_move(
                self.benchmark.fen,
                self.benchmark.blunder,
                traits=PersonalityTraits(tone="humorous"),
            )
        )
        self.assertTrue(response.natural_language_response.startswith("Yikes!"))

    def test_explain_move_intent_routes_to_the_coach(self):
        """Asking why a move failed runs the blunder pipeline."""
        self.set_evaluations(before=0.2, after=4.5)
        response = asyncio.run(
            self.agent.process_request(
                "why was that move bad?",
                fen=self.benchmark.fen,
                complexity=ComplexityLevel.INTERMEDIATE,
                context={"played_move": self.benchmark.blunder},
            )
        )
        self.assertEqual(response.intent, IntentType.EXPLAIN_MOVE)
        self.assertIn("fork your King and Rook", response.natural_language_response)


if __name__ == "__main__":
    unittest.main()
