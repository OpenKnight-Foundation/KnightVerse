# ---- Builder: install the package and its deps into an isolated venv ----
FROM python:3.11-slim AS builder
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential \
    && rm -rf /var/lib/apt/lists/*

RUN python -m venv /opt/venv
ENV PATH=/opt/venv/bin:$PATH

# Copy the package metadata *and* source before installing.
COPY pyproject.toml README.md ./
COPY gpu_worker ./gpu_worker
RUN pip install --no-cache-dir --upgrade pip \
    && pip install --no-cache-dir .

# ---- Runtime ----
FROM python:3.11-slim
WORKDIR /app

# Stockfish engine binary (Debian installs it at /usr/games/stockfish).
RUN apt-get update \
    && apt-get install -y --no-install-recommends stockfish \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m appuser
COPY --from=builder --chown=appuser:appuser /opt/venv /opt/venv
COPY --chown=appuser:appuser . .

ENV PATH=/opt/venv/bin:$PATH \
    PYTHONUNBUFFERED=1 \
    ENGINE_BACKEND=stockfish \
    STOCKFISH_PATH=/usr/games/stockfish \
    AGENT_WS_HOST=0.0.0.0 \
    AGENT_WS_PORT=8765

USER appuser
EXPOSE 8765
CMD ["python", "main.py"]
