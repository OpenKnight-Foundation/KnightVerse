"""Headless round-robin bot tournament simulator (minimal first pass).

See issue AI-26: this is a small, self-contained starting point, not the
full async 100+ concurrency implementation described in the issue.
"""
from __future__ import annotations

import itertools
import random
from dataclasses import dataclass, field


@dataclass
class BotResult:
    name: str
    elo: float = 1500.0
    wins: int = 0
    draws: int = 0
    losses: int = 0


def simulate_game(elo_a: float, elo_b: float) -> str:
    """Return 'a', 'b', or 'draw' using a simple Elo expected-score model."""
    expected_a = 1 / (1 + 10 ** ((elo_b - elo_a) / 400))
    roll = random.random()
    if roll < expected_a - 0.1:
        return "a"
    if roll > expected_a + 0.1:
        return "b"
    return "draw"


def run_round_robin(bot_names: list[str], games_per_pair: int = 1) -> dict[str, BotResult]:
    """Run a small round-robin tournament and return per-bot results."""
    results = {name: BotResult(name=name) for name in bot_names}

    for a, b in itertools.combinations(bot_names, 2):
        for _ in range(games_per_pair):
            outcome = simulate_game(results[a].elo, results[b].elo)
            if outcome == "a":
                results[a].wins += 1
                results[b].losses += 1
            elif outcome == "b":
                results[b].wins += 1
                results[a].losses += 1
            else:
                results[a].draws += 1
                results[b].draws += 1

    return results


if __name__ == "__main__":
    import json
    import time
    
    bots = ["Stockfish-16", "LC0-BT4", "Maia-1900", "Random-Bot"]
    results = run_round_robin(bots, games_per_pair=2)
    
    print("\n--- Tournament Results ---")
    for name, result in results.items():
        total = result.wins + result.draws + result.losses
        win_rate = (result.wins / total * 100) if total > 0 else 0
        print(f"{name}: {result.wins}W {result.draws}D {result.losses}L (Win Rate: {win_rate:.1f}%)")
    
    # Output results for CI artifact
    with open("tournament_results.json", "w") as f:
        json.dump({
            "timestamp": time.time(),
            "bots": bots,
            "games_per_pair": 2,
            "results": {
                name: {
                    "elo": result.elo,
                    "wins": result.wins,
                    "draws": result.draws,
                    "losses": result.losses,
                    "win_rate": (result.wins / (result.wins + result.draws + result.losses) * 100) if (result.wins + result.draws + result.losses) > 0 else 0
                }
                for name, result in results.items()
            }
        }, f, indent=2)
