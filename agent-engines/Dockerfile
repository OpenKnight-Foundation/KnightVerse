FROM python:3.11-slim as builder
WORKDIR /app
RUN apt-get update && apt-get install -y build-essential
COPY pyproject.toml .
RUN pip install --user .

FROM python:3.11-slim
WORKDIR /app
COPY --from=builder /root/.local /root/.local
ENV PATH=/root/.local/bin:$PATH
COPY . .
RUN useradd -m appuser && chown -R appuser /app
USER appuser
CMD ["python", "main.py"]
