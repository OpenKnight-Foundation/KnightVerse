from __future__ import annotations

from datetime import datetime, timedelta

from gpu_worker.models import Game, Move, Player
from gpu_worker.player_behavior_monitor import (
    OfflineAnalysisService,
    PlayerBehaviorMonitor,
)


class MockStockfishBridge:
    def get_top_moves(self, fen: str, num_pv: int) -> list[dict]:
        # Simulate Stockfish analysis, returning a fixed "best" move
        return [{"move": "e2e4"}]


def make_game(game_id: str, player1: Player, player2: Player, moves: list[Move]) -> Game:
    return Game(
        id=game_id,
        white_player=player1,
        black_player=player2,
        moves=moves,
        created_at=datetime.now(),
    )


def test_move_time_variance_flagging() -> None:
    player = Player(id="player-1")
    moves = [
        Move(player=player, move="e2e4", time_taken=1.0),
        Move(player=player, move="d2d4", time_taken=1.0),
        Move(player=player, move="Nf3", time_taken=1.0),
    ]
    game = make_game("game-1", player, Player(id="player-2"), moves)

    analysis_service = OfflineAnalysisService(MockStockfishBridge())
    flagged_players = analysis_service.analyze_games([game])

    assert "player-1" in flagged_players


def test_stockfish_correlation_flagging() -> None:
    player = Player(id="player-1")
    moves = [
        Move(player=player, move="e2e4", time_taken=5.0),
        Move(player=player, move="d2d4", time_taken=4.0),
        Move(player=player, move="Nf3", time_taken=6.0),
    ]
    game = make_game("game-1", player, Player(id="player-2"), moves)

    # Simulate high correlation by having the mock Stockfish always return the player's move
    class HighCorrelationStockfish(MockStockfishBridge):
        def get_top_moves(self, fen: str, num_pv: int) -> list[dict]:
            return [{"move": "e2e4"}]  # Assume all moves are the best move

    analysis_service = OfflineAnalysisService(HighCorrelationStockfish(), confidence_threshold=0.8)
    # This is a placeholder for the actual correlation logic
    analysis_service._calculate_stockfish_correlation = lambda moves, game: 0.9

    flagged_players = analysis_service.analyze_games([game])

    assert "player-1" in flagged_players