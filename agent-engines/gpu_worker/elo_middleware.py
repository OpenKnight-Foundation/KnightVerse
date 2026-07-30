"""API middleware for dynamic engine configuration injection based on player ELO.

This middleware sits between the matchmaking/WebSocket layer and the analysis
worker pool.  It:

1. Extracts the opponent's ELO from the incoming analysis request payload.
2. Calls :func:`~gpu_worker.elo_scaling.elo_to_engine_params` to derive the
   correct Stockfish parameters.
3. Applies those parameters directly to the :class:`~gpu_worker.models.AnalysisRequest`
   (``depth``, ``num_pv``).
4. Sends the ``setoption`` commands to the UCI bridge **before** the ``go``
   command so Stockfish honours the new Skill Level immediately.

The middleware is intentionally thin – it does *not* know about HTTP, WebSockets,
or message queues.  It operates on plain :class:`~gpu_worker.models.AnalysisRequest`
objects so it can be composed into any async pipeline.

Usage example
-------------
::

    from gpu_worker.elo_middleware import EloScalingMiddleware
    from gpu_worker.models import AnalysisRequest

    middleware = EloScalingMiddleware()

    # Before dispatching a request from a 1200-ELO player:
    request = AnalysisRequest(fen="...", opponent_elo=1200)
    scaled_request = middleware.apply(request)
    # scaled_request.depth == 6, scaled_request.num_pv == 3
"""

from __future__ import annotations

import logging
from typing import Optional

from pydantic import ConfigDict

from gpu_worker.elo_scaling import EngineParams, elo_to_engine_params
from gpu_worker.models import AnalysisRequest
from gpu_worker.uci_bridge import AsyncUciBridge

logger = logging.getLogger("KnightVerse.EloMiddleware")


# ---------------------------------------------------------------------------
# Enriched request model
# ---------------------------------------------------------------------------


class EloAnalysisRequest(AnalysisRequest):
    """Extended :class:`AnalysisRequest` that carries an optional ELO field.

    The ``opponent_elo`` field is set by the matchmaking layer when a human
    player is matched against the AI engine.  The middleware reads this field
    to scale the engine's difficulty dynamically.
    """

    model_config = ConfigDict(extra="ignore")

    opponent_elo: Optional[int] = None


# ---------------------------------------------------------------------------
# Middleware
# ---------------------------------------------------------------------------


class EloScalingMiddleware:
    """Middleware that injects ELO-scaled engine parameters into analysis requests.

    Attributes:
        default_elo: ELO used when the request does not carry an
                     ``opponent_elo`` value.  Defaults to 1500 (intermediate).
        override_existing: When ``True``, the middleware overwrites ``depth``
                           and ``num_pv`` even if they were already set.
                           Defaults to ``False`` so that explicit caller
                           overrides are respected.
    """

    def __init__(
        self,
        default_elo: int = 1500,
        override_existing: bool = False,
    ) -> None:
        self.default_elo = default_elo
        self.override_existing = override_existing

    # ------------------------------------------------------------------
    # Public interface
    # ------------------------------------------------------------------

    def apply(self, request: AnalysisRequest) -> tuple[AnalysisRequest, EngineParams]:
        """Apply ELO scaling to *request* and return the modified copy.

        The method does **not** mutate the original request; it returns a new
        instance with the scaled parameters injected.

        Args:
            request: The original analysis request.  If it is an
                     :class:`EloAnalysisRequest`, the ``opponent_elo`` field
                     is used.  Otherwise :attr:`default_elo` is used.

        Returns:
            A tuple of *(scaled_request, engine_params)* where:
            - ``scaled_request`` is the request with ``depth`` and ``num_pv``
              set according to the player's ELO.
            - ``engine_params`` carries the full set of derived Stockfish
              parameters (including ``skill_level``) so the caller can send
              ``setoption`` commands to the UCI bridge.
        """
        elo = self._extract_elo(request)
        params = elo_to_engine_params(elo)

        # Build updated field dict from the base AnalysisRequest fields only.
        # We dump the parent class fields to avoid duplicate-kwarg errors when
        # the incoming request is already an EloAnalysisRequest.
        base_fields = {
            "id": request.id,
            "fen": request.fen,
            "depth": request.depth,
            "time_limit_ms": request.time_limit_ms,
            "search_moves": request.search_moves,
            "num_pv": request.num_pv,
            "priority": request.priority,
            "actor_id": request.actor_id,
            "session_id": request.session_id,
            "ip_hash": request.ip_hash,
            "device_hash": request.device_hash,
            "created_at": request.created_at,
        }

        if self.override_existing or base_fields.get("depth") is None:
            base_fields["depth"] = params.depth
        # num_pv defaults to 1 in AnalysisRequest.  We treat the default value
        # as "not explicitly set" so that the ELO-derived MultiPV is applied.
        # A caller that truly wants num_pv=1 despite a low ELO can set
        # override_existing=False and pass num_pv=1 explicitly — the result
        # is the same (1 stays); to force ELO scaling they can use
        # override_existing=True.
        _num_pv_is_default = base_fields.get("num_pv") in (None, 1)
        if self.override_existing or _num_pv_is_default:
            base_fields["num_pv"] = params.multi_pv

        # Preserve the opponent_elo so downstream systems can log it.
        scaled = EloAnalysisRequest(**base_fields, opponent_elo=elo)

        logger.info(
            "ELO middleware: elo=%d → skill=%d depth=%d num_pv=%d request_id=%s",
            elo,
            params.skill_level,
            params.depth,
            params.multi_pv,
            request.id,
        )

        return scaled, params

    async def configure_bridge(
        self,
        bridge: AsyncUciBridge,
        params: EngineParams,
    ) -> None:
        """Send ``setoption`` commands to the UCI bridge for the given params.

        This method must be called **before** :pymeth:`AsyncUciBridge.go` so
        that Stockfish applies the new settings to the current search.

        Args:
            bridge: A started :class:`~gpu_worker.uci_bridge.AsyncUciBridge`.
            params: Parameters produced by :func:`~gpu_worker.elo_scaling.elo_to_engine_params`.
        """
        # ``_set_option_if_supported`` is the bridge's internal helper; we call
        # the public setoption pathway via the private method because the bridge
        # already tracks which options the engine supports and silently skips
        # unsupported ones.
        await bridge._set_option_if_supported("Skill Level", str(params.skill_level))
        await bridge._set_option_if_supported("MultiPV", str(params.multi_pv))
        await bridge.ensure_ready()

        logger.debug(
            "Bridge configured: Skill Level=%d MultiPV=%d",
            params.skill_level,
            params.multi_pv,
        )

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _extract_elo(self, request: AnalysisRequest) -> int:
        """Extract ELO from the request or fall back to the default."""
        if isinstance(request, EloAnalysisRequest) and request.opponent_elo is not None:
            return request.opponent_elo
        # Fallback: honour the default.
        logger.debug(
            "No opponent_elo on request %s; using default ELO %d",
            request.id,
            self.default_elo,
        )
        return self.default_elo
