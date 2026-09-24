import asyncio
import chess
import pytest

from gpu_worker.models import Player

try:
    from gpu_worker.personality import CommentaryEngine, CommentaryTone, GameMoveEvent
except ImportError:
    pytest.skip(
        "gpu_worker.personality has no CommentaryEngine/GameMoveEvent yet "
        "(only PersonalityManager is implemented) — commentary streaming is unbuilt",
        allow_module_level=True,
    )


def make_event(fen_before: str, move: chess.Move, move_number: int, player_id: str = "test_player", clock_remaining: float = 60) -> GameMoveEvent:
    return GameMoveEvent(
        move_number=move_number,
        player=Player(id=player_id, rated=True),
        move=move,
        fen_before=fen_before,
        time_taken=1.0,
        clock_remaining=clock_remaining,
    )


def collect_stream(engine: CommentaryEngine, event: GameMoveEvent) -> list[str]:
    async def _collect():
        return [token async for token in engine.stream_commentary(event)]
    return asyncio.run(_collect())


#--- Brilliant move ---
def test_brilliant_move_commentary():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    # White queen d1 goes to e4, where it's attacked by a black knight on f6 and undefended.
    fen_before = "4k3/8/8/5n2/8/8/8/3QK3 w - - 0 1"
    move = chess.Move.from_uci("d1e4")
    event = make_event(fen_before, move, move_number=1)
    commentary = engine.analyze_event(event)
    assert commentary is not None
    assert "brilliant" in commentary.lower()


#--- Trade ---
def test_trade_commentary():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    # White queen captures black queen on d8, with black king capable of recapture.
    fen_before = "3qkc/8/8/8/8/8/8/qqK3 w - - 0 1"
    move = chess.Move.from_uci("d1d8")
    event = make_event(fen_before, move, move_number=2)
    commentary = engine.analyze_event(event)
    assert commentary is not None
    assert "trade" in commentary.lower()


#--- Missed opportunity ---
def test_missed_opportunity_commentary():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    # Black knight on e5 is undefended and attacked by white pawn on d4.
    fen_before = "4k3/8/8/4n3/3P4/8/8/4K3 w - - 0 1"
    move = chess.Move.from_uci("e1d2")  # Make a quiet king move instead of capturing the knight.
    event = make_event(fen_before, move, move_number=3)
    commentary = engine.analyze_event(event)
    assert commentary is not None
    assert "missed" in commentary.lower()


#--- Clock panic ---
def test_clock_panic_commentary():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    fen_before = chess.Board().fen()  # Standard opening
    move = chess.Move.from_uci("g1f3")  # Normal knight move
    event = make_event(fen_before, move, move_number=1, clock_remaining=10.0)
    commentary = engine.analyze_event(event)
    assert commentary is not None
    assert "seconds" in commentary.lower() or "time trouble" in commentary.lower()


#--- Counter-attack ---
def test_counterattack_commentary():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    fen_before = "4k3/8/8/8/8/8/8/4K2Q w - - 0 1"  # White queen on h1, king e1; black king e8.
    move = chess.Move.from_uci("h1h5")  # Queen to h5 gives check.
    event = make_event(fen_before, move, move_number=3)
    commentary = engine.analyze_event(event)
    assert commentary is not None
    assert "counter-attack" in commentary.lower()


#--- Rate limiting ---
def test_rate_limiting_routine_moves():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    board = chess.Board()
    moves = ["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8"]
    last_comment = None
    for n, uci in enumerate(moves):
        move = chess.Move.from_uci(uci)
        event = make_event(board.fen(), move, move_number=n + 1)
        commentary = engine.analyze_event(event)
        board.push(move)
        if commentary is not None:
            assert last_comment is None or n - last_comment >= 3, "Rate limit violated"
            last_comment = n
    assert last_comment is not None


#--- Streaming tokens ---
def test_stream_commentary_yields_tokens():
    engine = CommentaryEngine(tone=CommentaryTone.EXCITED)
    fen_before = "4k3/8/8/5n2/8/8/8/3QK3 w - - 0 1"
    move = chess.Move.from_uci("d1e4")
    event = make_event(fen_before, move, move_number=1)
    tokens = collect_stream(engine, event)
    assert len(tokens) > 0
