from __future__ import annotations

import asyncio
from contextlib import suppress
from dataclasses import dataclass
from itertools import count

from gpu_worker.models import AnalysisRequest, AnalysisResult
from gpu_worker.pool import WorkerPool


@dataclass(slots=True)
class _QueuedRequest:
    """Internal representation of a request waiting to be batched."""

    request: AnalysisRequest
    future: asyncio.Future[AnalysisResult]


class BatchAnalyzer:
    """Batch multiple position analyses for efficient GPU utilization.

    Requests are queued individually via `submit`, but a background loop
    groups them into batches (by size or a flush timeout, whichever comes
    first) before handing them to the worker pool, so the GPU can process
    several positions per call instead of one at a time.
    """

    def __init__(
        self,
        pool: WorkerPool,
        batch_size: int = 16,
        flush_interval_ms: int = 100,
    ) -> None:
        if batch_size < 1:
            raise ValueError("batch_size must be positive")
        if flush_interval_ms < 1:
            raise ValueError("flush_interval_ms must be positive")
        self.pool = pool
        self.batch_size = batch_size
        self.flush_interval_seconds = flush_interval_ms / 1000.0
        # Monotonically increasing tiebreaker so equal-priority requests are
        # still ordered FIFO (PriorityQueue needs a total order on entries).
        self._sequence = count()
        # Min-heap ordered by (-priority, sequence): higher AnalysisRequest
        # priority sorts first, and insertion order breaks ties.
        self._queue: asyncio.PriorityQueue[tuple[int, float, _QueuedRequest]] = (
            asyncio.PriorityQueue()
        )
        self._shutdown_event = asyncio.Event()
        # Background task that continuously forms and dispatches batches.
        self._task = asyncio.create_task(self._batch_loop())

    async def submit(self, request: AnalysisRequest) -> AnalysisResult:
        """Add one request to the batch queue and await its result."""

        # The future is resolved later by _batch_loop once this request's
        # batch has been processed by the worker pool.
        future: asyncio.Future[AnalysisResult] = asyncio.get_running_loop().create_future()
        queued = _QueuedRequest(request=request, future=future)
        await self._queue.put(
            (-request.priority, next(self._sequence), queued)
        )
        return await future

    async def submit_batch(
        self, requests: list[AnalysisRequest]
    ) -> list[AnalysisResult]:
        """Submit many requests and await all results."""

        # Each request still goes through the normal per-request queueing/
        # batching path; this just fans out the submits and awaits them together.
        return await asyncio.gather(*(self.submit(request) for request in requests))

    async def shutdown(self) -> None:
        """Stop the background batching loop."""

        self._shutdown_event.set()
        self._task.cancel()
        # Cancelling the loop while it's awaiting the queue/pool raises
        # CancelledError inside it; swallow that expected exception here.
        with suppress(asyncio.CancelledError):
            await self._task

    async def _batch_loop(self) -> None:
        """Collect requests until the batch is full or the flush timer expires."""

        while not self._shutdown_event.is_set():
            # Block until at least one request is available, then start the
            # flush-interval clock from that first arrival.
            _, _, first_item = await self._queue.get()
            batch = [first_item]
            deadline = asyncio.get_running_loop().time() + self.flush_interval_seconds

            # Keep pulling more requests until the batch is full or time runs out.
            while len(batch) < self.batch_size:
                timeout = deadline - asyncio.get_running_loop().time()
                if timeout <= 0:
                    break
                try:
                    _, _, item = await asyncio.wait_for(self._queue.get(), timeout=timeout)
                    batch.append(item)
                except asyncio.TimeoutError:
                    # No new request arrived before the deadline; ship what we have.
                    break

            # Dispatch the whole batch to the worker pool concurrently, capturing
            # per-item exceptions instead of letting one failure kill the batch.
            results = await asyncio.gather(
                *(self.pool.submit(item.request) for item in batch),
                return_exceptions=True,
            )
            # Resolve each request's future with its own result or exception,
            # so callers of `submit` see failures isolated to their own request.
            for item, result in zip(batch, results, strict=True):
                if isinstance(result, Exception):
                    item.future.set_exception(result)
                else:
                    item.future.set_result(result)