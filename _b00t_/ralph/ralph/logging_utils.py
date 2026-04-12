"""Logging helpers for Ralph automation."""

from __future__ import annotations

import logging
import sys
from typing import Final

LOGGER_NAME: Final[str] = "ralph"
LOG_FORMAT: Final[str] = "%(message)s"


def configure_logging(level: int = logging.INFO) -> logging.Logger:
    """Configure a stderr logger for Ralph."""

    logger = logging.getLogger(LOGGER_NAME)
    if not logger.handlers:
        handler = logging.StreamHandler(sys.stderr)
        handler.setFormatter(logging.Formatter(LOG_FORMAT))
        logger.addHandler(handler)
    logger.setLevel(level)
    logger.propagate = False
    return logger


def log_success(logger: logging.Logger, message: str) -> None:
    """Log a success message."""

    logger.info("✅ %s", message)


def log_info(logger: logging.Logger, message: str) -> None:
    """Log an informational message."""

    logger.info("ℹ️ %s", message)


def log_warning(logger: logging.Logger, message: str) -> None:
    """Log a warning message."""

    logger.warning("⚠️ %s", message)


def log_error(logger: logging.Logger, message: str, exc: Exception | None = None) -> None:
    """Log an error message with optional traceback."""

    if exc is None:
        logger.error("❌ %s", message)
        return
    logger.error("❌ %s", message, exc_info=(type(exc), exc, exc.__traceback__))


__all__ = [
    "configure_logging",
    "log_error",
    "log_info",
    "log_success",
    "log_warning",
]
