"""Unit tests for Ralph logging utilities."""

from __future__ import annotations

import logging
from io import StringIO

import pytest

from ralph.logging_utils import (
    configure_logging,
    log_error,
    log_info,
    log_success,
    log_warning,
)


@pytest.fixture
def logger_with_capture() -> tuple[logging.Logger, StringIO]:
    """Create a logger with captured output."""
    logger = logging.getLogger("test_ralph")
    logger.handlers.clear()
    logger.setLevel(logging.DEBUG)

    stream = StringIO()
    handler = logging.StreamHandler(stream)
    handler.setFormatter(logging.Formatter("%(message)s"))
    logger.addHandler(handler)
    logger.propagate = False

    return logger, stream


def test_configure_logging_creates_logger() -> None:
    """Test configure_logging() creates a logger."""
    logger = configure_logging()

    assert logger is not None
    assert logger.name == "ralph"
    assert logger.level == logging.INFO
    assert len(logger.handlers) > 0


def test_configure_logging_custom_level() -> None:
    """Test configure_logging() with custom level."""
    logger = configure_logging(level=logging.DEBUG)

    assert logger.level == logging.DEBUG


def test_configure_logging_singleton() -> None:
    """Test configure_logging() returns same logger instance."""
    logger1 = configure_logging()
    logger2 = configure_logging()

    assert logger1 is logger2


def test_log_success(logger_with_capture: tuple[logging.Logger, StringIO]) -> None:
    """Test log_success() outputs with emoji."""
    logger, stream = logger_with_capture

    log_success(logger, "Operation succeeded")

    output = stream.getvalue()
    assert "✅" in output
    assert "Operation succeeded" in output


def test_log_info(logger_with_capture: tuple[logging.Logger, StringIO]) -> None:
    """Test log_info() outputs with emoji."""
    logger, stream = logger_with_capture

    log_info(logger, "Informational message")

    output = stream.getvalue()
    assert "ℹ️" in output
    assert "Informational message" in output


def test_log_warning(logger_with_capture: tuple[logging.Logger, StringIO]) -> None:
    """Test log_warning() outputs with emoji."""
    logger, stream = logger_with_capture

    log_warning(logger, "Warning message")

    output = stream.getvalue()
    assert "⚠️" in output
    assert "Warning message" in output


def test_log_error_without_exception(
    logger_with_capture: tuple[logging.Logger, StringIO],
) -> None:
    """Test log_error() without exception."""
    logger, stream = logger_with_capture

    log_error(logger, "Error message")

    output = stream.getvalue()
    assert "❌" in output
    assert "Error message" in output


def test_log_error_with_exception(
    logger_with_capture: tuple[logging.Logger, StringIO],
) -> None:
    """Test log_error() with exception traceback."""
    logger, stream = logger_with_capture

    try:
        raise ValueError("Test exception")
    except ValueError as exc:
        log_error(logger, "Error occurred", exc)

    output = stream.getvalue()
    assert "❌" in output
    assert "Error occurred" in output
    assert "ValueError: Test exception" in output
    assert "Traceback" in output


def test_log_error_with_chained_exception(
    logger_with_capture: tuple[logging.Logger, StringIO],
) -> None:
    """Test log_error() with chained exception."""
    logger, stream = logger_with_capture

    try:
        try:
            raise ValueError("Original error")
        except ValueError as exc:
            raise RuntimeError("Wrapped error") from exc
    except RuntimeError as exc:
        log_error(logger, "Chained error occurred", exc)

    output = stream.getvalue()
    assert "❌" in output
    assert "Chained error occurred" in output
    assert "RuntimeError: Wrapped error" in output
    # Should show the chain
    assert "ValueError: Original error" in output


def test_logging_to_stderr() -> None:
    """Test that configure_logging() outputs to stderr."""
    import sys

    logger = configure_logging()

    # Check that handler writes to stderr
    for handler in logger.handlers:
        if isinstance(handler, logging.StreamHandler):
            assert handler.stream == sys.stderr
