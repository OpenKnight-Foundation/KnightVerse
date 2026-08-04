from __future__ import annotations

from collections import defaultdict
from datetime import datetime
import statistics

from gpu_worker.models import Game, Move, Player


class PlayerBehaviorMonitor:
    """
    Tracks player move times and decisions to detect suspicious patterns.
    """

    def __init__(self) -> None:
        self.player_move_times: dict[str, list[float]] = defaultdict(list)
        self.player_move_decisions: dict[str, list[Move]] = defaultdict(list)

    def record_move(self, player: Player, move: Move, move_time: float) -> None:
        """
        Records a single move for a player.
        """
        self.player_move_times[player.id].append(move_time)
        self.player_move_decisions[player.id].append(move)

    def get_move_time_variance(self, player_id: str) -> float | None:
        """
        Calculates the variance of move times for a player.
        """
        move_times = self.player_move_times.get(player_id)
        if not move_times or len(move_times) < 2:
            return None
        return statistics.variance(move_times)


class OfflineAnalysisService:
    """
    Analyzes recent rated games to detect cheating.
    """

    def __init__(self, stockfish_bridge, confidence_threshold: float = 0.85) -> None:
        self.stockfish_bridge = stockfish_bridge
        self.confidence_threshold = confidence_threshold
        self.player_monitor = PlayerBehaviorMonitor()

    def analyze_games(self, games: list[Game]) -> list[str]:
        """
        Analyzes a list of games and returns a list of flagged player IDs.
        """
        flagged_players = []
        for game in games:
            for move in game.moves:
                self.player_monitor.record_move(move.player, move, move.time_taken)

            for player in [game.white_player, game.black_player]:
                if self._is_suspicious(player, game):
                    flagged_players.append(player.id)
        return flagged_players

    def _is_suspicious(self, player: Player, game: Game) -> bool:
        """
        Checks if a player's behavior in a game is suspicious.
        """
        move_time_variance = self.player_monitor.get_move_time_variance(player.id)
        if move_time_variance is not None and move_time_variance < 0.1:
            return True  # Flag for unusually consistent move times

        player_moves = self.player_monitor.player_move_decisions[player.id]
        stockfish_correlation = self._calculate_stockfish_correlation(player_moves, game)
        return stockfish_correlation > self.confidence_threshold

    def _calculate_stockfish_correlation(self, player_moves: list[Move], game: Game) -> float:
        """
        Calculates the correlation between player moves and Stockfish's top choices.
        """
        # This is a placeholder for the actual Stockfish analysis logic.
        # In a real implementation, you would use the stockfish_bridge to get
        # the top moves from Stockfish for each position in the game and
        # compare them to the player's moves.
        return 0.0