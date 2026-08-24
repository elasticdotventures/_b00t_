# syntax=docker/dockerfile:1
ARG PYTHON_VERSION=3.10

FROM python:${PYTHON_VERSION}-slim AS deps
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/
WORKDIR /build
COPY pyproject.toml ./
COPY b00t_docling_serve/ b00t_docling_serve/
COPY README.md ./
RUN uv sync --no-dev --extra docling --extra nats-service

# On-demand Docling NATS Micro Service — no exposed port (NATS-only,
# outbound connection), no perpetual process: see nats_service.py's
# idle-exit and ../deploy/quadlet/docling-nats-service.container's lack
# of an [Install] section.
FROM python:${PYTHON_VERSION}-slim AS docling-nats-service
LABEL org.opencontainers.image.title="b00t-docling-serve" \
      org.opencontainers.image.description="On-demand Docling extraction exposed as a NATS Micro Service (ledgrrr-docling)" \
      org.opencontainers.image.vendor="PromptExecution"

RUN groupadd -r docling && useradd -r -g docling -u 1000 docling
WORKDIR /app
COPY --from=deps --chown=docling:docling /build/.venv /app/.venv
COPY --chown=docling:docling b00t_docling_serve/ /app/b00t_docling_serve/
COPY --chown=docling:docling pyproject.toml README.md /app/

ENV PATH="/app/.venv/bin:$PATH" \
    PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1 \
    IDLE_TIMEOUT_SECONDS=300

USER docling
CMD ["python", "-m", "b00t_docling_serve.nats_service"]
