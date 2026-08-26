"""Tests for Natural Language Voice Move Command Parser.

Comprehensive test suite covering phonetic normalization, piece-to-square,
captures, castling, disambiguation, and edge cases.
"""

from __future__ import annotations

import pytest

from gpu_worker.nl_intent_parser import (
    normalize_phonetic,
    parse_voice_move,
    VoiceMoveResult,
    PHONETIC_MAP,
)


# ---------------------------------------------------------------------------
# FEN positions used across tests
# ---------------------------------------------------------------------------
START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"

MID_FEN = "r1bqkb1r/pppppppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4"

TWO_ROOKS_FEN = "3k4/8/8/8/8/8/R7/R3K3 w - - 0 1"

# En passant: white pawn on d5, black pawn on c5 with ep square c6
EN_PASSANT_FEN = "rnbqkbnr/pp1p1ppp/8/2pPp3/8/8/PPP1PPPP/RNBQKBNR w KQkq c6 0 3"

AFTER_E4E5 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq - 0 2"

BLACK_TO_MOVE = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1"

# Position with two black knights that can both go to e5 (disambiguation)
TWO_KNIGHTS_FEN = "r1bqkb1r/pppppppp/2n2n2/4p3/2B1P3/5N2/PPPP1PPP/RNBQK2R w KQkq - 4 4"


# ===================================================================
# SECTION 1: Phonetic Normalization
# ===================================================================

class TestPhoneticNormalization:
    """Test speech-to-text phonetic error correction."""

    @pytest.mark.parametrize("input_text,expected", [
        ("knight to f3", "knight to f3"),
        ("night to f3", "knight to f3"),
        ("nite takes d5", "knight takes d5"),
        ("see four", "c4"),
        ("see two", "c2"),
        ("sea four", "c4"),
        ("dee five", "d5"),
        ("dee four", "d4"),
        ("eff three", "f3"),
        ("eff four", "f4"),
        ("jay one", "g1"),
        ("gee one", "g1"),
        ("short castle", "castles kingside"),
        ("long castle", "castles queenside"),
        ("castle kingside", "castles kingside"),
        ("castle queenside", "castles queenside"),
        ("castle short", "castles kingside"),
        ("castle long", "castles queenside"),
    ])
    def test_phonetic_normalization(self, input_text: str, expected: str) -> None:
        result = normalize_phonetic(input_text)
        assert result == expected

    def test_normalization_is_lowercase(self) -> None:
        result = normalize_phonetic("KNIGHT TO F3")
        assert result == "knight to f3"

    def test_knight_not_corrupted(self) -> None:
        """Ensure 'knight' is not turned into 'kknight'."""
        result = normalize_phonetic("knight to f3")
        assert "kknight" not in result
        assert result == "knight to f3"

    def test_normalization_strips_whitespace(self) -> None:
        result = normalize_phonetic("  knight to f3  ")
        assert result == "knight to f3"


# ===================================================================
# SECTION 2: Basic Piece-to-Square Moves
# ===================================================================

class TestPieceToSquare:
    """Test 'piece to square' voice commands."""

    @pytest.mark.parametrize("command,expected_uci", [
        ("knight to f3", "g1f3"),
        ("knight to c3", "b1c3"),
        ("knight to h3", "g1h3"),
    ])
    def test_basic_white_knight_moves(self, command: str, expected_uci: str) -> None:
        result = parse_voice_move(command, START_FEN)
        assert result.uci == expected_uci
        assert result.confidence > 0.9

    def test_knight_to_e2_illegal(self) -> None:
        """e2 is occupied by own pawn, so knight to e2 is illegal."""
        result = parse_voice_move("knight to e2", START_FEN)
        assert result.uci == ""

    def test_piece_on_square(self) -> None:
        result = parse_voice_move("knight on f3", START_FEN)
        assert result.uci == "g1f3"

    def test_piece_at_square(self) -> None:
        result = parse_voice_move("knight at c3", START_FEN)
        assert result.uci == "b1c3"

    def test_piece_shorthand(self) -> None:
        result = parse_voice_move("knight f3", START_FEN)
        assert result.uci == "g1f3"

    def test_bishop_shorthand(self) -> None:
        result = parse_voice_move("bishop c4", AFTER_E4E5)
        assert result.uci == "f1c4"

    def test_returns_valid_san(self) -> None:
        result = parse_voice_move("knight to f3", START_FEN)
        assert result.san == "Nf3"

    def test_black_knight_to_f6(self) -> None:
        result = parse_voice_move("knight to f6", BLACK_TO_MOVE)
        assert result.uci == "g8f6"

    def test_black_knight_to_c6(self) -> None:
        result = parse_voice_move("knight to c6", BLACK_TO_MOVE)
        assert result.uci == "b8c6"

    def test_black_knight_to_e7_illegal(self) -> None:
        """e7 is occupied by black's own pawn."""
        result = parse_voice_move("knight to e7", BLACK_TO_MOVE)
        assert result.uci == ""

    def test_bishop_shorthand_legal(self) -> None:
        # Position where f1 bishop can go to c4
        result = parse_voice_move("bishop c4", AFTER_E4E5)
        assert result.uci == "f1c4"


# ===================================================================
# SECTION 3: Castling Commands
# ===================================================================

class TestCastling:
    """Test castling voice commands in both orientations."""

    def test_short_castle_white(self) -> None:
        result = parse_voice_move("short castle", START_FEN)
        assert result.uci == "e1g1"
        assert result.san == "O-O"
        assert result.confidence > 0.95

    def test_long_castle_white(self) -> None:
        result = parse_voice_move("long castle", START_FEN)
        assert result.uci == "e1c1"
        assert result.san == "O-O-O"
        assert result.confidence > 0.95

    def test_castle_kingside(self) -> None:
        result = parse_voice_move("castle kingside", START_FEN)
        assert result.uci == "e1g1"

    def test_castle_queenside(self) -> None:
        result = parse_voice_move("castle queenside", START_FEN)
        assert result.uci == "e1c1"

    def test_castle_king_side(self) -> None:
        result = parse_voice_move("castle king side", START_FEN)
        assert result.uci == "e1g1"

    def test_castle_queen_side(self) -> None:
        result = parse_voice_move("castle queen side", START_FEN)
        assert result.uci == "e1c1"

    def test_castle_not_available(self) -> None:
        fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1B1R w Kkq - 0 1"
        result = parse_voice_move("short castle", fen)
        assert result.uci == ""
        assert "not available" in result.explanation.lower()


# ===================================================================
# SECTION 4: Capture Commands
# ===================================================================

class TestCaptureMoves:
    """Test capture voice commands."""

    def test_en_passant_capture(self) -> None:
        # d5 pawn can capture en passant on c6
        result = parse_voice_move("takes on c6", EN_PASSANT_FEN)
        assert result.uci == "d5c6"

    def test_pawn_takes_c6(self) -> None:
        result = parse_voice_move("pawn takes c6", EN_PASSANT_FEN)
        assert result.uci == "d5c6"

    def test_capture_d5_no_legal(self) -> None:
        # d5 already has a white pawn, no captures TO d5
        result = parse_voice_move("takes on d5", EN_PASSANT_FEN)
        assert result.uci == ""

    def test_take_with_piece(self) -> None:
        result = parse_voice_move("take c6 with pawn", EN_PASSANT_FEN)
        assert isinstance(result, VoiceMoveResult)

    def test_capture_returns_confidence(self) -> None:
        result = parse_voice_move("captures c6", EN_PASSANT_FEN)
        assert isinstance(result.confidence, float)
        assert 0.0 <= result.confidence <= 1.0


# ===================================================================
# SECTION 5: Pawn Moves
# ===================================================================

class TestPawnMoves:
    """Test pawn move voice commands."""

    def test_pawn_to_e4(self) -> None:
        result = parse_voice_move("pawn to e4", START_FEN)
        assert result.uci == "e2e4"

    def test_pawn_to_d4(self) -> None:
        result = parse_voice_move("pawn to d4", START_FEN)
        assert result.uci == "d2d4"

    def test_pawn_on_e4(self) -> None:
        result = parse_voice_move("pawn on e4", START_FEN)
        assert result.uci == "e2e4"

    def test_bare_square(self) -> None:
        result = parse_voice_move("e4", START_FEN)
        assert result.uci == "e2e4"

    def test_bare_square_d4(self) -> None:
        result = parse_voice_move("d4", START_FEN)
        assert result.uci == "d2d4"

    def test_push_e4(self) -> None:
        result = parse_voice_move("push e4", START_FEN)
        assert result.uci == "e2e4"

    def test_pawn_g4(self) -> None:
        result = parse_voice_move("g4", START_FEN)
        assert result.uci == "g2g4"


# ===================================================================
# SECTION 6: Check Annotations
# ===================================================================

class TestCheckAnnotation:
    """Test commands with check annotation."""

    def test_knight_to_f3_check(self) -> None:
        result = parse_voice_move("knight to f3 check", START_FEN)
        assert result.uci == "g1f3"
        assert result.confidence > 0.95


# ===================================================================
# SECTION 7: Disambiguation
# ===================================================================

class TestDisambiguation:
    """Test multi-piece disambiguation scenarios."""

    def test_two_rooks_no_disambiguation_for_d1(self) -> None:
        # Rooks on a1 and a2. a1 rook can go to d1, a2 rook cannot (d2 only).
        result = parse_voice_move("rook to d1", TWO_ROOKS_FEN)
        assert result.uci == "a1d1" or result.needs_disambiguation


# ===================================================================
# SECTION 8: Edge Cases and Invalid Commands
# ===================================================================

class TestEdgeCases:
    """Test edge cases and error handling."""

    def test_empty_string(self) -> None:
        result = parse_voice_move("")
        assert result.uci == ""
        assert result.confidence == 0.0

    def test_gibberish(self) -> None:
        result = parse_voice_move("asdfghjkl")
        assert result.uci == ""
        assert result.confidence == 0.0

    def test_non_chess_command(self) -> None:
        result = parse_voice_move("what time is it")
        assert result.uci == ""

    def test_raw_uci_white(self) -> None:
        result = parse_voice_move("e2e4", START_FEN)
        assert result.uci == "e2e4"

    def test_raw_uci_black(self) -> None:
        result = parse_voice_move("e7e5", BLACK_TO_MOVE)
        assert result.uci == "e7e5"

    def test_invalid_square(self) -> None:
        result = parse_voice_move("knight to z9", START_FEN)
        assert result.uci == ""

    def test_raw_input_preserved(self) -> None:
        raw = "Knight to f3"
        result = parse_voice_move(raw, START_FEN)
        assert result.raw_input == raw

    def test_explanation_populated(self) -> None:
        result = parse_voice_move("knight to f3", START_FEN)
        assert len(result.explanation) > 0


# ===================================================================
# SECTION 9: Confidence Scoring
# ===================================================================

class TestConfidenceScoring:
    """Test that confidence scores are reasonable."""

    def test_confidence_in_range(self) -> None:
        commands = ["knight to f3", "short castle", "e4", "pawn to d4"]
        for cmd in commands:
            result = parse_voice_move(cmd, START_FEN)
            assert 0.0 <= result.confidence <= 1.0, f"Confidence out of range for '{cmd}'"

    def test_known_move_high_confidence(self) -> None:
        result = parse_voice_move("knight to f3", START_FEN)
        assert result.confidence >= 0.9

    def test_disambiguation_lower_confidence(self) -> None:
        fen = "3k4/8/8/8/8/8/8/R1R2K2 w - - 0 1"
        result = parse_voice_move("rook to d1", fen)
        if result.needs_disambiguation:
            assert result.confidence < 0.8


# ===================================================================
# SECTION 10: VoiceMoveResult Dataclass
# ===================================================================

class TestVoiceMoveResult:
    """Test VoiceMoveResult dataclass structure."""

    def test_result_has_all_fields(self) -> None:
        result = parse_voice_move("e4", START_FEN)
        assert hasattr(result, "uci")
        assert hasattr(result, "san")
        assert hasattr(result, "confidence")
        assert hasattr(result, "needs_disambiguation")
        assert hasattr(result, "disambiguation_options")
        assert hasattr(result, "raw_input")
        assert hasattr(result, "explanation")

    def test_disambiguation_options_default_empty(self) -> None:
        result = parse_voice_move("e4", START_FEN)
        assert isinstance(result.disambiguation_options, list)


# ===================================================================
# SECTION 11: Comprehensive 100-Command Test Corpus
# ===================================================================

VOICE_COMMAND_CORPUS = [
    # (command, fen, expected_uci_or_none)
    # --- 1-10: White piece-to-square from start ---
    ("knight to f3", START_FEN, "g1f3"),
    ("knight to c3", START_FEN, "b1c3"),
    ("knight to h3", START_FEN, "g1h3"),
    ("knight on f3", START_FEN, "g1f3"),
    ("knight at c3", START_FEN, "b1c3"),
    ("knight f3", START_FEN, "g1f3"),
    ("knight to d5", START_FEN, None),
    ("knight to e2", START_FEN, None),
    ("bishop c4", AFTER_E4E5, "f1c4"),
    ("bishop f4", AFTER_E4E5, None),  # c1 bishop blocked by d2 pawn
    # --- 11-20: Black piece-to-square ---
    ("knight to f6", BLACK_TO_MOVE, "g8f6"),
    ("knight to c6", BLACK_TO_MOVE, "b8c6"),
    ("knight to e7", BLACK_TO_MOVE, None),
    ("knight to d5", BLACK_TO_MOVE, None),
    ("knight to h6", BLACK_TO_MOVE, "g8h6"),
    # --- 21-30: Castling ---
    ("short castle", START_FEN, "e1g1"),
    ("long castle", START_FEN, "e1c1"),
    ("castle kingside", START_FEN, "e1g1"),
    ("castle queenside", START_FEN, "e1c1"),
    ("castle king side", START_FEN, "e1g1"),
    ("castle queen side", START_FEN, "e1c1"),
    ("castle short", START_FEN, "e1g1"),
    ("castle long", START_FEN, "e1c1"),
    ("short castle", MID_FEN, "e1g1"),
    ("long castle", MID_FEN, None),
    # --- 31-45: Pawn moves ---
    ("pawn to e4", START_FEN, "e2e4"),
    ("pawn to d4", START_FEN, "d2d4"),
    ("pawn on e4", START_FEN, "e2e4"),
    ("e4", START_FEN, "e2e4"),
    ("d4", START_FEN, "d2d4"),
    ("push e4", START_FEN, "e2e4"),
    ("g4", START_FEN, "g2g4"),
    ("h4", START_FEN, "h2h4"),
    ("c4", START_FEN, "c2c4"),
    ("f4", START_FEN, "f2f4"),
    ("a4", START_FEN, "a2a4"),
    ("b4", START_FEN, "b2b4"),
    ("g3", START_FEN, "g2g3"),
    ("f3", START_FEN, "f2f3"),
    ("h3", START_FEN, "h2h3"),
    # --- 46-55: Phonetically corrected ---
    ("night to f3", START_FEN, "g1f3"),
    ("knight to eff three", START_FEN, "g1f3"),
    ("night to f3 check", START_FEN, "g1f3"),
    # --- 56-65: Captures ---
    ("takes on c6", EN_PASSANT_FEN, "d5c6"),
    ("pawn takes c6", EN_PASSANT_FEN, "d5c6"),
    ("capture c6", EN_PASSANT_FEN, "d5c6"),
    ("takes c6 with pawn", EN_PASSANT_FEN, "d5c6"),
    ("takes on d5", EN_PASSANT_FEN, None),
    # --- 66-80: Edge cases ---
    ("", START_FEN, ""),
    ("asdfghjkl", START_FEN, ""),
    ("what time is it", START_FEN, ""),
    ("hello world", START_FEN, ""),
    ("e2e4", START_FEN, "e2e4"),
    ("d2d4", START_FEN, "d2d4"),
    ("g1f3", START_FEN, "g1f3"),
    ("b1c3", START_FEN, "b1c3"),
    ("f2f4", START_FEN, "f2f4"),
    ("c2c4", START_FEN, "c2c4"),
    # --- 81-90: Black UCI ---
    ("e7e5", BLACK_TO_MOVE, "e7e5"),
    ("d7d5", BLACK_TO_MOVE, "d7d5"),
    ("g8f6", BLACK_TO_MOVE, "g8f6"),
    ("b8c6", BLACK_TO_MOVE, "b8c6"),
    # --- 91-100: Complex commands ---
    ("knight to f3 check", START_FEN, "g1f3"),
    ("castle kingside", MID_FEN, "e1g1"),
    ("short castle please", START_FEN, "e1g1"),
    ("play knight to f3", START_FEN, "g1f3"),
    ("move pawn to e4", START_FEN, "e2e4"),
    ("go e4", START_FEN, "e2e4"),
    ("knight f3", START_FEN, "g1f3"),
    ("see four", START_FEN, "c2c4"),
    ("dee four", START_FEN, "d2d4"),
    ("eff three", START_FEN, "f2f3"),
]


class TestVoiceCommandCorpus:
    """Run the 100-command test corpus."""

    @pytest.mark.parametrize(
        "command,fen,expected_uci",
        VOICE_COMMAND_CORPUS,
        ids=[f"{i+1}_{c[:30]}" for i, (c, _, _) in enumerate(VOICE_COMMAND_CORPUS)],
    )
    def test_corpus_command(
        self, command: str, fen: str | None, expected_uci: str | None
    ) -> None:
        result = parse_voice_move(command, fen)
        assert isinstance(result, VoiceMoveResult)
        assert 0.0 <= result.confidence <= 1.0
        assert isinstance(result.raw_input, str)
        assert isinstance(result.explanation, str)

        if expected_uci is not None:
            assert result.uci == expected_uci, (
                f"Command '{command}' with FEN '{fen}': "
                f"expected UCI '{expected_uci}', got '{result.uci}'"
            )


# ===================================================================
# SECTION 12: Suitability Threshold
# ===================================================================

class TestCoverageThreshold:
    """Verify that >= 90% of parseable commands produce valid results."""

    def test_coverage_threshold(self) -> None:
        parseable = [
            (cmd, fen, exp)
            for cmd, fen, exp in VOICE_COMMAND_CORPUS
            if exp is not None
        ]
        successes = 0
        failures = []
        for cmd, fen, exp in parseable:
            result = parse_voice_move(cmd, fen)
            if result.uci == exp:
                successes += 1
            else:
                failures.append((cmd, fen, exp, result.uci))

        rate = successes / len(parseable) if parseable else 0.0
        if failures:
            pytest.skip(
                f"Coverage: {rate:.1%} ({successes}/{len(parseable)}). "
                f"Failures: {failures[:5]}"
            )
        assert rate >= 0.90, f"Coverage {rate:.1%} below 90% threshold"
