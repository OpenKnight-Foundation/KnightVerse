from __future__ import annotations

import chess.polyglot
import chess

class OpeningBook:
    """Reader for a Polyglot opening book."""

    def __init__(self, book_path: str):
        self.book_path = book_path

    def find_move(self, board: chess.Board) -> chess.polyglot.Move | None:
        """Find a book move for the given board position."""
        try:
            with chess.polyglot.open_reader(self.book_path) as reader:
                return reader.find(board)
        except (FileNotFoundError, IndexError):
            return None