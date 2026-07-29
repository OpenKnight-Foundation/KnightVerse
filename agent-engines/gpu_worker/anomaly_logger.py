"""Structured logging sink for bot-farm anomaly alerts.

Sends anomaly reports as JSON to stdout (ingestible by ELK/Datadog agents)
and optionally to a configurable alerting webhook.
"""

from __future__ import annotations

import json
import logging
import os
from datetime import datetime, timezone
from typing import Any

from gpu_worker.anomaly import BotFarmReport

logger = logging.getLogger("KnightVerse.AnomalyLogger")

# Environment-configurable webhook URL
ANOMALY_WEBHOOK_URL = os.environ.get("KNIGHTVERSE_ANOMALY_WEBHOOK_URL", "")
ANOMALY_WEBHOOK_TIMEOUT_S = int(os.environ.get("KNIGHTVERSE_ANOMALY_WEBHOOK_TIMEOUT_S", "10"))


def _serialize_report(report: BotFarmReport) -> dict[str, Any]:
    """Serialize a BotFarmReport into a JSON-ready dictionary."""
    return {
        "type": "anomaly_report",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "risk_level": report.risk_level.value,
        "score": report.score,
        "findings": [
            {
                "risk_level": finding.risk_level.value,
                "score": finding.score,
                "reason": finding.reason,
                "affected_actor_ids": finding.affected_actor_ids,
                "evidence": finding.evidence,
            }
            for finding in report.findings
        ],
        "window_start": report.window_start.isoformat() if report.window_start else None,
        "window_end": report.window_end.isoformat() if report.window_end else None,
        "window_seconds": report.window_seconds,
        "bucket_size_seconds": report.bucket_size_seconds,
        "event_count": report.event_count,
        "distinct_actor_count": report.distinct_actor_count,
    }


def log_anomaly_report(report: BotFarmReport) -> None:
    """Emit the anomaly report as a structured JSON log line to stdout.

    This produces a single JSON line per report so log aggregators
    (ELK, Datadog, Loki) can index and query fields directly.
    """
    if not report.findings:
        return  # Only log reports with actual findings

    payload = _serialize_report(report)

    # Use a dedicated logger at WARNING level so it stands out in dashboards
    logger.warning("ANOMALY_REPORT %s", json.dumps(payload))


async def send_anomaly_webhook(report: BotFarmReport) -> bool:
    """POST the anomaly report to the configured alerting webhook.

    Returns True if the webhook was sent successfully, False otherwise.
    Skips sending if no webhook URL is configured.

    The aiohttp dependency is imported lazily so that the module remains
    importable even when aiohttp is not installed (e.g. in environments
    that only use stdout logging).
    """
    if not ANOMALY_WEBHOOK_URL or not report.findings:
        return True  # Nothing to send, not an error

    try:
        import aiohttp  # noqa: PLC0415 — lazy import for optional webhook support
    except ImportError:
        logger.warning(
            "Cannot send anomaly webhook: aiohttp is not installed. "
            "Install it with 'pip install aiohttp' to enable webhook alerts."
        )
        return False

    payload = _serialize_report(report)

    try:
        timeout = aiohttp.ClientTimeout(total=ANOMALY_WEBHOOK_TIMEOUT_S)
        async with aiohttp.ClientSession(timeout=timeout) as session:
            async with session.post(
                ANOMALY_WEBHOOK_URL,
                json=payload,
                headers={"Content-Type": "application/json"},
            ) as response:
                if response.status < 200 or response.status >= 300:
                    logger.error(
                        "Anomaly webhook returned status %d: %s",
                        response.status,
                        await response.text(),
                    )
                    return False
                return True
    except Exception:
        logger.exception("Failed to send anomaly webhook to %s", ANOMALY_WEBHOOK_URL)
        return False
