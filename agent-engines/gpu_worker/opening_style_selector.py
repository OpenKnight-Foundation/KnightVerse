"""Personality-filtered opening style selector (minimal first pass).

See issue AI-27. Maps bot personalities to a small set of ECO codes so the
opening book lookup can be biased toward a style; falls back to no filter
if the personality is unknown.
"""
from __future__ import annotations

PERSONALITY_ECO_CODES: dict[str, list[str]] = {
    "aggressive": ["C33", "B90", "D45", "C51"],  # King's Gambit, Sicilian Dragon-ish, Danish, Evans
    "positional": ["B10", "D06", "C65"],  # Caro-Kann, QGD, Berlin Defense
    "hypermodern": ["E90", "E20", "B02"],  # King's Indian, Nimzo-Indian, Alekhine Defense
}


def eco_codes_for_personality(personality: str) -> list[str]:
    """Return the ECO codes preferred by a bot personality, or [] if unknown."""
    return PERSONALITY_ECO_CODES.get(personality.lower(), [])


def filter_moves_by_style(
    candidate_moves: list[tuple[str, str]], personality: str
) -> list[tuple[str, str]]:
    """Filter (move, eco_code) candidates down to a personality's preferred openings.

    Falls back to the full candidate list if none match, so bots never end
    up out of book.
    """
    preferred = set(eco_codes_for_personality(personality))
    if not preferred:
        return candidate_moves

    matches = [m for m in candidate_moves if m[1] in preferred]
    return matches or candidate_moves
