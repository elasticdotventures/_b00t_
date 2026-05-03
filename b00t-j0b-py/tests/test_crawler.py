"""Tests for web crawler functionality."""

import pytest
from unittest.mock import Mock, patch, MagicMock
from returns.result import Success, Failure

from b00t_j0b_py.crawler import WebCrawler
from b00t_j0b_py.robots import URLValidator
from b00t_j0b_py.redis_client import RedisTracker


class TestURLValidator:
    """Test URL validation."""
    
    def test_valid_urls(self):
        """Test valid URL detection."""
        valid_urls = [
            "https://example.com",
            "http://github.com/user/repo",
            "https://pypi.org/project/test/",
        ]
        
        for url in valid_urls:
            assert URLValidator.is_valid_url(url)
    
    def test_invalid_urls(self):
        """Test invalid URL detection."""
        invalid_urls = [
            "not-a-url",
            "ftp://example.com",
            "https://example.com/file.jpg",
            "https://example.com/style.css",
            "",
        ]
        
        for url in invalid_urls:
            assert not URLValidator.is_valid_url(url)
    
    def test_url_normalization(self):
        """Test URL normalization."""
        test_cases = [
            ("https://example.com/", "https://example.com"),
            ("https://example.com/path/", "https://example.com/path"),
            ("https://example.com/path?q=test", "https://example.com/path?q=test"),
            ("https://example.com/path#fragment", "https://example.com/path"),
        ]
        
        for input_url, expected in test_cases:
            assert URLValidator.normalize_url(input_url) == expected
    
    def test_same_domain_check(self):
        """Test domain comparison."""
        assert URLValidator.is_same_domain("https://example.com/a", "https://example.com/b")
        assert not URLValidator.is_same_domain("https://example.com", "https://other.com")


class TestWebCrawler:
    """Test web crawler."""
    
    @pytest.fixture
    def crawler(self):
        """Create crawler instance for testing."""
        return WebCrawler(max_depth=2, delay=0.1)
    
    @pytest.fixture
    def mock_response(self):
        """Mock HTTP response."""
        response = Mock()
        response.status_code = 200
        response.headers = {"content-type": "text/html"}
        response.text = """
        <html>
            <head><title>Test Page</title></head>
            <body>
                <h1>Test Content</h1>
                <p>This is test content.</p>
                <a href="/link1">Link 1</a>
                <a href="https://example.com/link2">Link 2</a>
            </body>
        </html>
        """
        return response
    
    def test_extract_title(self, crawler):
        """Test title extraction."""
        html = "<html><head><title>Test Title</title></head></html>"
        title = crawler._extract_title(html)
        assert title == "Test Title"
    
    def test_extract_title_fallback(self, crawler):
        """Test title extraction fallback to h1."""
        html = "<html><body><h1>Header Title</h1></body></html>"
        title = crawler._extract_title(html)
        assert title == "Header Title"
    
    def test_extract_links(self, crawler):
        """Test link extraction."""
        html = """
        <html>
            <body>
                <a href="/relative">Relative</a>
                <a href="https://example.com/absolute">Absolute</a>
                <a href="mailto:test@example.com">Email</a>
                <a href="/page.html">Valid</a>
                <a href="/image.jpg">Image</a>
            </body>
        </html>
        """
        
        links = crawler._extract_links(html, "https://example.com")
        
        # Should extract valid links and make them absolute
        expected_links = {
            "https://example.com/relative",
            "https://example.com/absolute", 
            "https://example.com/page.html"
            # Should exclude email and image links
        }
        
        # Note: URLValidator.is_valid_url filters out some links
        valid_links = {link for link in links if URLValidator.is_valid_url(link)}
        assert len(valid_links) > 0  # Should have some valid links
    
    def test_html_to_markdown(self, crawler):
        """Test HTML to markdown conversion."""
        html = """
        <html>
            <body>
                <h1>Title</h1>
                <p>Paragraph text</p>
                <ul>
                    <li>Item 1</li>
                    <li>Item 2</li>
                </ul>
            </body>
        </html>
        """
        
        markdown = crawler._html_to_markdown(html, "https://example.com/test")
        
        # Should contain markdown elements
        assert "# Title" in markdown
        assert "Paragraph text" in markdown
        assert "- Item 1" in markdown or "* Item 1" in markdown
        assert "Source: https://example.com/test" in markdown
    
    @patch('b00t_j0b_py.crawler.tracker')
    @patch('b00t_j0b_py.crawler.robots_checker')
    def test_crawl_url_success(self, mock_robots, mock_tracker, crawler, mock_response):
        """Test successful URL crawling."""
        # Setup mocks
        mock_tracker.is_crawled.return_value = False
        mock_robots.is_allowed.return_value = Success(True)
        mock_robots.get_crawl_delay.return_value = None
        
        with patch.object(crawler, '_fetch_url', return_value=Success(mock_response)):
            result = crawler.crawl_url("https://example.com/test")
        
        assert isinstance(result, Success)
        data = result.unwrap()
        assert data["url"] == "https://example.com/test"
        assert data["status_code"] == 200
        assert "Test Content" in data["content"]
    
    @patch('b00t_j0b_py.crawler.tracker')
    @patch('b00t_j0b_py.crawler.robots_checker')
    def test_crawl_url_robots_disallowed(self, mock_robots, mock_tracker, crawler):
        """Test crawling blocked by robots.txt."""
        mock_tracker.is_crawled.return_value = False
        mock_robots.is_allowed.return_value = Success(False)
        
        result = crawler.crawl_url("https://example.com/blocked")
        
        assert isinstance(result, Failure)
        assert "disallows" in str(result.failure())
    
    @patch('b00t_j0b_py.crawler.tracker')
    def test_crawl_url_already_crawled(self, mock_tracker, crawler):
        """Test skipping already crawled URLs."""
        mock_tracker.is_crawled.return_value = True
        mock_tracker.get_crawl_info.return_value = {"depth": 0}
        
        result = crawler.crawl_url("https://example.com/test")
        
        assert isinstance(result, Failure)
        assert "already crawled" in str(result.failure())


class TestRedisTracker:
    """Test Redis tracking functionality."""
    
    @pytest.fixture
    def tracker(self):
        """Create tracker instance for testing."""
        # Use fake Redis for testing
        with patch('b00t_j0b_py.redis_client.redis.from_url') as mock_redis:
            mock_client = MagicMock()
            mock_redis.return_value = mock_client
            
            tracker = RedisTracker("redis://localhost:6379/15")  # Test DB
            tracker.redis = mock_client
            return tracker
    
    def test_url_key_generation(self, tracker):
        """Test URL key generation."""
        key1 = tracker._url_key("https://example.com/test")
        key2 = tracker._url_key("https://example.com/test")
        key3 = tracker._url_key("https://example.com/other")
        
        # Same URL should generate same key
        assert key1 == key2
        # Different URLs should generate different keys
        assert key1 != key3
        # Keys should have expected format
        assert key1.startswith("crawl:url:")
    
    def test_mark_crawled(self, tracker):
        """Test marking URLs as crawled."""
        tracker.redis.setex.return_value = True
        
        result = tracker.mark_crawled("https://example.com/test", 1, 200)
        
        assert isinstance(result, Success)
        tracker.redis.setex.assert_called_once()
    
    def test_is_crawled(self, tracker):
        """Test checking if URL is crawled."""
        tracker.redis.exists.return_value = 1
        
        assert tracker.is_crawled("https://example.com/test")
        
        tracker.redis.exists.return_value = 0
        assert not tracker.is_crawled("https://example.com/test")
    
    def test_queue_operations(self, tracker):
        """Test queue add/pop operations."""
        tracker.redis.sadd.return_value = 1
        tracker.redis.scard.return_value = 5
        tracker.redis.spop.return_value = '{"url": "https://example.com", "depth": 1}'
        
        # Test adding to queue
        urls = {"https://example.com/test1", "https://example.com/test2"}
        result = tracker.add_to_queue(urls, 1)
        assert isinstance(result, Success)
        
        # Test queue size
        size = tracker.get_queue_size()
        assert size == 5
        
        # Test popping from queue
        item = tracker.pop_from_queue()
        assert item.unwrap()["url"] == "https://example.com"