from __future__ import annotations

from pydantic import BaseModel

class MaiaConfig(BaseModel):
    """
    Configuration for a Maia Chess model.
    """
    name: str
    path: str
    elo: int