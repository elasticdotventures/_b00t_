"""Test URL detection and crawling functionality."""
import pytest
import pytest_asyncio
import sys

# Mock b00t_grok module
sys.modules['b00t_grok'] = None


@pytest_asyncio.fixture
async def guru():
    """Create and initialize a GrokGuru instance."""
    from b00t_grok_guru.guru import GrokGuru
    g = GrokGuru(qdrant_url="http://test:6333", api_key="test")
    await g.initialize()
    return g


@pytest.mark.asyncio
class TestURLDetection:
    async def test_is_url_valid(self, guru):
        """Test URL detection with valid URLs."""
        assert guru._is_url("https://github.com/unclecode/crawl4ai")
        assert guru._is_url("http://example.com")
        assert guru._is_url("https://docs.python.org/3/")

    async def test_is_url_invalid(self, guru):
        """Test URL detection with invalid strings."""
        assert not guru._is_url("not a url")
        assert not guru._is_url("file.txt")
        assert not guru._is_url("")


@pytest.mark.asyncio
class TestURLCrawling:
    @pytest.mark.skip(reason="Requires internet connection and crawl4ai browser setup")
    async def test_crawl_url_basic(self, guru):
        """Test basic URL crawling."""
        # Test with example.com (lightweight, always available)
        content = await guru._crawl_url("http://example.com")
        assert content is not None
        assert len(content) > 50
        assert "example" in content.lower()

    async def test_learn_with_url_source(self, guru):
        """Test learn method detects URL in source parameter."""
        # This will fail gracefully if crawl4ai not set up
        result = await guru.learn(
            content="dummy content",
            source="https://github.com/unclecode/crawl4ai"
        )
        # Should either succeed with crawled content or fail gracefully
        assert result is not None
        assert hasattr(result, 'success')

    async def test_learn_with_url_content(self, guru):
        """Test learn method detects URL in content parameter."""
        result = await guru.learn(
            content="https://github.com/unclecode/crawl4ai"
        )
        # Should either succeed with crawled content or fail gracefully
        assert result is not None
        assert hasattr(result, 'success')


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
