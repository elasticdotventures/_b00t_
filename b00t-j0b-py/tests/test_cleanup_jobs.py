"""Tests for cleanup job functionality."""

import pytest
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, MagicMock
import json

from b00t_j0b_py.jobs import cleanup_old_data_job
from b00t_j0b_py.redis_client import RedisTracker


@pytest.fixture
def mock_redis():
    """Mock Redis connection."""
    redis_mock = MagicMock()
    redis_mock.scan.return_value = (0, [])  # Empty scan by default
    redis_mock.get.return_value = None
    redis_mock.delete.return_value = 1
    redis_mock.scard.return_value = 0
    return redis_mock


@pytest.fixture
def mock_tracker(mock_redis):
    """Mock RedisTracker."""
    tracker = Mock(spec=RedisTracker)
    tracker.redis = mock_redis
    tracker.get_stats.return_value = {
        "crawled_urls": 100,
        "cached_robots": 10,
        "cached_content": 50,
        "default_queue": 0,
        "high_queue": 0,
        "low_queue": 0,
    }
    return tracker


@pytest.fixture
def mock_rq_job():
    """Mock RQ job."""
    job = Mock()
    job.id = "test-job-123"
    return job


class TestCleanupJob:
    """Tests for cleanup_old_data_job."""

    @patch("b00t_j0b_py.jobs.get_current_job")
    @patch("b00t_j0b_py.jobs.tracker")
    @patch("b00t_j0b_py.jobs.get_redis_connection")
    @patch("b00t_j0b_py.jobs.get_all_queues")
    def test_cleanup_basic_execution(
        self,
        mock_get_queues,
        mock_get_redis,
        mock_tracker_patch,
        mock_get_job,
        mock_tracker,
        mock_redis,
        mock_rq_job,
    ):
        """Test basic cleanup job execution."""
        # Setup mocks
        mock_get_job.return_value = mock_rq_job
        mock_tracker_patch.get_stats = mock_tracker.get_stats
        mock_get_redis.return_value = mock_redis
        mock_get_queues.return_value = []

        # Execute cleanup
        result = cleanup_old_data_job(max_age_days=7)

        # Assertions
        assert result["status"] == "success"
        assert "stats_before" in result
        assert "stats_after" in result
        assert "cleanup_results" in result
        assert result["job_id"] == "test-job-123"

    @patch("b00t_j0b_py.jobs.get_current_job")
    @patch("b00t_j0b_py.jobs.tracker")
    @patch("b00t_j0b_py.jobs.get_redis_connection")
    @patch("b00t_j0b_py.jobs.get_all_queues")
    def test_cleanup_old_crawl_data(
        self,
        mock_get_queues,
        mock_get_redis,
        mock_tracker_patch,
        mock_get_job,
        mock_tracker,
        mock_redis,
        mock_rq_job,
    ):
        """Test cleanup of old crawled URL data."""
        # Setup mocks
        mock_get_job.return_value = mock_rq_job
        mock_tracker_patch.get_stats = mock_tracker.get_stats
        mock_get_redis.return_value = mock_redis
        mock_get_queues.return_value = []

        # Create old crawl data
        old_timestamp = (datetime.utcnow() - timedelta(days=10)).isoformat()
        crawl_data = json.dumps({
            "url": "https://example.com",
            "depth": 0,
            "status_code": 200,
            "crawled_at": old_timestamp,
        })

        # Mock scan to return old keys
        mock_redis.scan.side_effect = [
            (10, ["crawl:url:abc123"]),  # First scan iteration
            (0, []),  # Second scan iteration (cursor=0 means done)
        ]
        mock_redis.get.return_value = crawl_data

        # Execute cleanup
        result = cleanup_old_data_job(max_age_days=7)

        # Assertions
        assert result["status"] == "success"
        assert result["cleanup_results"]["crawl_data_cleaned"] == 1
        mock_redis.delete.assert_called()

    @patch("b00t_j0b_py.jobs.get_current_job")
    @patch("b00t_j0b_py.jobs.tracker")
    @patch("b00t_j0b_py.jobs.get_redis_connection")
    @patch("b00t_j0b_py.jobs.get_all_queues")
    def test_cleanup_skips_recent_data(
        self,
        mock_get_queues,
        mock_get_redis,
        mock_tracker_patch,
        mock_get_job,
        mock_tracker,
        mock_redis,
        mock_rq_job,
    ):
        """Test cleanup skips recently crawled data."""
        # Setup mocks
        mock_get_job.return_value = mock_rq_job
        mock_tracker_patch.get_stats = mock_tracker.get_stats
        mock_get_redis.return_value = mock_redis
        mock_get_queues.return_value = []

        # Create recent crawl data
        recent_timestamp = (datetime.utcnow() - timedelta(days=2)).isoformat()
        crawl_data = json.dumps({
            "url": "https://example.com",
            "depth": 0,
            "status_code": 200,
            "crawled_at": recent_timestamp,
        })

        # Mock scan to return recent keys
        mock_redis.scan.return_value = (0, ["crawl:url:xyz789"])
        mock_redis.get.return_value = crawl_data

        # Execute cleanup
        result = cleanup_old_data_job(max_age_days=7)

        # Assertions
        assert result["status"] == "success"
        assert result["cleanup_results"]["crawl_data_cleaned"] == 0
        # Should not delete recent data
        mock_redis.delete.assert_not_called()

    @patch("b00t_j0b_py.jobs.get_current_job")
    @patch("b00t_j0b_py.jobs.tracker")
    @patch("b00t_j0b_py.jobs.get_redis_connection")
    @patch("b00t_j0b_py.jobs.get_all_queues")
    def test_cleanup_with_failed_jobs(
        self,
        mock_get_queues,
        mock_get_redis,
        mock_tracker_patch,
        mock_get_job,
        mock_tracker,
        mock_redis,
        mock_rq_job,
    ):
        """Test cleanup of failed jobs registry."""
        # Setup mocks
        mock_get_job.return_value = mock_rq_job
        mock_tracker_patch.get_stats = mock_tracker.get_stats
        mock_get_redis.return_value = mock_redis

        # Mock queue with failed jobs
        mock_queue = Mock()
        mock_failed_registry = Mock()
        mock_failed_registry.__len__ = Mock(return_value=2)
        mock_failed_registry.get_job_ids.return_value = ["failed-1", "failed-2"]

        mock_queue.failed_job_registry = mock_failed_registry
        mock_queue.finished_job_registry = Mock()
        mock_queue.finished_job_registry.get_job_ids.return_value = []

        mock_get_queues.return_value = [mock_queue]

        # Mock failed jobs
        with patch("b00t_j0b_py.jobs.Job") as mock_job_class:
            mock_job1 = Mock()
            mock_job1.ended_at = datetime.utcnow() - timedelta(days=10)

            mock_job2 = Mock()
            mock_job2.ended_at = datetime.utcnow() - timedelta(days=8)

            mock_job_class.fetch.side_effect = [mock_job1, mock_job2]

            # Execute cleanup
            result = cleanup_old_data_job(max_age_days=7, clean_failed_jobs=True)

            # Assertions
            assert result["status"] == "success"
            assert result["cleanup_results"]["failed_jobs_cleaned"] == 2

    @patch("b00t_j0b_py.jobs.get_current_job")
    @patch("b00t_j0b_py.jobs.tracker")
    @patch("b00t_j0b_py.jobs.get_redis_connection")
    def test_cleanup_handles_errors_gracefully(
        self,
        mock_get_redis,
        mock_tracker_patch,
        mock_get_job,
        mock_tracker,
        mock_redis,
        mock_rq_job,
    ):
        """Test cleanup handles errors gracefully."""
        # Setup mocks
        mock_get_job.return_value = mock_rq_job
        mock_tracker_patch.get_stats = mock_tracker.get_stats
        mock_get_redis.return_value = mock_redis

        # Force an error in scan
        mock_redis.scan.side_effect = Exception("Redis connection lost")

        # Execute cleanup - should not crash
        result = cleanup_old_data_job(max_age_days=7)

        # Should still return success but with errors logged
        assert result["status"] == "success"
        assert len(result["cleanup_results"]["errors"]) > 0

    @patch("b00t_j0b_py.jobs.get_current_job")
    @patch("b00t_j0b_py.jobs.tracker")
    def test_cleanup_without_rq_context(
        self,
        mock_tracker_patch,
        mock_get_job,
        mock_tracker,
    ):
        """Test cleanup can run outside RQ context (local mode)."""
        # No RQ job context
        mock_get_job.return_value = None
        mock_tracker_patch.get_stats = mock_tracker.get_stats

        with patch("b00t_j0b_py.jobs.get_redis_connection") as mock_get_redis, \
             patch("b00t_j0b_py.jobs.get_all_queues") as mock_get_queues:

            mock_redis = MagicMock()
            mock_redis.scan.return_value = (0, [])
            mock_get_redis.return_value = mock_redis
            mock_get_queues.return_value = []

            # Execute cleanup
            result = cleanup_old_data_job(max_age_days=7)

            # Should work with job_id="local"
            assert result["status"] == "success"
            assert result["job_id"] == "local"


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
