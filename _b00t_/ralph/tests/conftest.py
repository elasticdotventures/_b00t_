"""Pytest configuration and shared fixtures for Ralph tests."""

from __future__ import annotations

import pytest


@pytest.fixture(autouse=True)
def reset_logging() -> None:
    """Reset logging configuration between tests."""
    import logging

    # Clear handlers from the ralph logger
    logger = logging.getLogger("ralph")
    logger.handlers.clear()
    logger.setLevel(logging.NOTSET)
    logger.propagate = True
