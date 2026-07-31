from __future__ import annotations

import json
from typing import Optional

import redis

from gpu_worker.models import AnalysisResult

class RedisCache:
    """
    A Redis-based cache for storing analysis results.
    """

    def __init__(self, host: str, port: int, db: int):
        self.redis = redis.Redis(host=host, port=port, db=db)

    def get(self, key: str) -> Optional[AnalysisResult]:
        """Get an analysis result from the cache."""
        value = self.redis.get(key)
        if value:
            return AnalysisResult(**json.loads(value))
        return None

    def set(self, key: str, result: AnalysisResult, ttl: int) -> None:
        """Set an analysis result in the cache with a TTL."""
        self.redis.set(key, result.model_dump_json(), ex=ttl)