"""Tests for grok integration and post-processing pipeline."""

import pytest
from unittest.mock import Mock, patch, AsyncMock
import asyncio
from datetime import datetime
import json

from b00t_j0b_py.grok_integration import (
    AdvancedGrokProcessor,
    GrokIntegrationError,
    MockGrokGuru,
    process_crawl_result_job,
    process_batch_crawl_results_job,
    get_grok_processor
)
from b00t_j0b_py.advanced_chunking import ChunkType, ChunkingStrategy
from returns.result import Success, Failure


@pytest.mark.asyncio
class TestMockGrokGuru:
    """Test mock implementation when b00t-grok-guru is not available."""
    
    async def test_initialization(self):
        """Test mock grok guru initialization."""
        guru = MockGrokGuru()
        assert not guru.initialized
        
        await guru.initialize()
        assert guru.initialized
    
    async def test_learn_method(self):
        """Test mock learn method."""
        guru = MockGrokGuru()
        await guru.initialize()
        
        content = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph."
        result = await guru.learn("test-source", content)
        
        assert result.success
        assert result.chunks_created == 3
        assert result.source == "test-source"
        assert len(result.chunks) == 3


@pytest.mark.asyncio
class TestAdvancedGrokProcessor:
    """Test the main grok processor."""
    
    @pytest.fixture
    async def processor(self):
        """Create and initialize processor."""
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            proc = AdvancedGrokProcessor()
            await proc.initialize()
            return proc
    
    async def test_initialization(self):
        """Test processor initialization."""
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            processor = AdvancedGrokProcessor()
            
            assert not processor._initialized
            
            await processor.initialize()
            
            assert processor._initialized
            assert processor.grok_guru is not None
            assert processor.chunking_engine is not None
    
    async def test_initialization_error_handling(self):
        """Test error handling during initialization."""
        with patch('b00t_j0b_py.grok_integration.MockGrokGuru') as mock_guru_class:
            mock_guru = mock_guru_class.return_value
            mock_guru.initialize.side_effect = Exception("Init failed")
            
            processor = AdvancedGrokProcessor()
            
            with pytest.raises(GrokIntegrationError, match="Failed to initialize"):
                await processor.initialize()
    
    async def test_process_crawl_result_success(self, processor):
        """Test successful processing of crawl result."""
        crawl_result = {
            "url": "https://github.com/test/repo",
            "content": """
# Test Repository

This is a test repository with code examples.

```python
def hello_world():
    print("Hello, World!")
```

## Features

- Feature 1
- Feature 2
""",
            "content_type": "text/markdown",
            "status_code": 200,
            "depth": 0,
            "links": ["https://github.com/test/repo/issues"],
            "parsed_metadata": {
                "platform": "github",
                "title": "Test Repository",
                "tags": ["python", "example"]
            }
        }
        
        result = await processor.process_crawl_result(crawl_result)
        
        assert isinstance(result, Success)
        data = result.unwrap()
        
        assert data["status"] == "success"
        assert data["url"] == "https://github.com/test/repo"
        assert data["chunks_created"] > 0
        assert "structural" in data["strategies_used"] or "hybrid" in data["strategies_used"]
        assert "grok_response" in data
        assert "processing_stats" in data
        
        # Check processing stats
        stats = data["processing_stats"]
        assert "total_chars" in stats
        assert "total_words" in stats
        assert "chunk_types" in stats
        
        # Verify processor stats are updated
        assert processor.processed_urls == 1
        assert processor.total_chunks_created > 0
    
    async def test_process_empty_content(self, processor):
        """Test handling of empty or minimal content."""
        crawl_result = {
            "url": "https://example.com/empty",
            "content": "",
            "content_type": "text/plain"
        }
        
        result = await processor.process_crawl_result(crawl_result)
        
        assert isinstance(result, Success)
        data = result.unwrap()
        
        assert data["status"] == "skipped"
        assert data["reason"] == "content_too_short"
        assert data["chunks_created"] == 0
    
    async def test_process_small_content(self, processor):
        """Test handling of very small content."""
        crawl_result = {
            "url": "https://example.com/small", 
            "content": "Hi",  # Too small
            "content_type": "text/plain"
        }
        
        result = await processor.process_crawl_result(crawl_result)
        
        assert isinstance(result, Success)
        data = result.unwrap()
        
        assert data["status"] == "skipped"
        assert data["reason"] == "content_too_short"
    
    async def test_chunking_strategy_selection(self, processor):
        """Test that appropriate chunking strategies are selected."""
        # Test code-heavy content → Structural
        code_heavy_result = {
            "url": "https://github.com/test/code",
            "content": "```python\ncode\n```\n\n```rust\nmore code\n```",
            "content_type": "text/markdown",
            "parsed_metadata": {"platform": "github"}
        }
        
        result = await processor.process_crawl_result(code_heavy_result)
        data = result.unwrap()
        
        assert "structural" in data["strategies_used"]
        
        # Test long documentation → Hybrid
        long_doc_result = {
            "url": "https://docs.example.com/guide",
            "content": "# " + "Long documentation content. " * 200,  # Long content
            "content_type": "text/markdown",
            "parsed_metadata": {"platform": "docs"}
        }
        
        result = await processor.process_crawl_result(long_doc_result)
        data = result.unwrap()
        
        # Should use hybrid or semantic for long content
        assert any(strategy in ["hybrid", "semantic"] for strategy in data["strategies_used"])
    
    async def test_error_handling(self, processor):
        """Test error handling during processing."""
        # Mock the chunking engine to raise an error
        with patch.object(processor.chunking_engine, 'chunk_content', side_effect=Exception("Chunking failed")):
            crawl_result = {
                "url": "https://example.com/error",
                "content": "Some content",
                "content_type": "text/plain"
            }
            
            result = await processor.process_crawl_result(crawl_result)
            
            assert isinstance(result, Failure)
            assert processor.processing_errors == 1
    
    async def test_metadata_storage(self, processor):
        """Test that chunk relationships and metadata are stored."""
        crawl_result = {
            "url": "https://example.com/hierarchy",
            "content": """
# Main Section

Content under main section.

## Subsection

More content here.
""",
            "content_type": "text/markdown"
        }
        
        with patch.object(processor._global_processor.redis if hasattr(processor, '_global_processor') else Mock(), 'setex', return_value=True) as mock_setex:
            result = await processor.process_crawl_result(crawl_result)
            
            assert result.is_success()
            data = result.unwrap()
            
            assert "relationship_info" in data
            relationship_info = data["relationship_info"]
            assert "relationship_key" in relationship_info
            assert "hierarchical_chunks" in relationship_info
    
    async def test_processing_stats(self, processor):
        """Test processing statistics tracking."""
        initial_stats = processor.get_processing_stats()
        
        assert initial_stats["processed_urls"] == 0
        assert initial_stats["total_chunks_created"] == 0
        assert initial_stats["processing_errors"] == 0
        
        # Process a successful result
        crawl_result = {
            "url": "https://example.com/stats",
            "content": "# Test\n\nSome content for stats testing.",
            "content_type": "text/markdown"
        }
        
        await processor.process_crawl_result(crawl_result)
        
        updated_stats = processor.get_processing_stats()
        
        assert updated_stats["processed_urls"] == 1
        assert updated_stats["total_chunks_created"] > 0
        assert updated_stats["avg_chunks_per_url"] > 0


class TestJobFunctions:
    """Test RQ job functions."""
    
    @pytest.mark.asyncio
    async def test_process_crawl_result_job(self):
        """Test single crawl result job function."""
        crawl_result = {
            "url": "https://example.com/job",
            "content": "# Job Test\n\nContent for job testing.",
            "content_type": "text/markdown"
        }
        
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            result = await process_crawl_result_job(crawl_result)
            
            assert result["status"] == "success"
            assert result["job_type"] == "advanced_chunking"
            assert "chunks_created" in result
    
    def test_process_batch_crawl_results_job(self):
        """Test batch processing job function."""
        crawl_results = [
            {
                "url": f"https://example.com/batch-{i}",
                "content": f"# Batch Test {i}\n\nContent for batch testing.",
                "content_type": "text/markdown"
            }
            for i in range(3)
        ]
        
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            result = process_batch_crawl_results_job(crawl_results)
            
            assert result["batch_size"] == 3
            assert result["successful"] >= 0
            assert result["failed"] >= 0
            assert result["successful"] + result["failed"] == 3
            assert "processing_stats" in result
            assert "results" in result
    
    @pytest.mark.asyncio
    async def test_global_processor_singleton(self):
        """Test global processor singleton behavior."""
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            # Clear any existing global processor
            import b00t_j0b_py.grok_integration as gi
            gi._global_processor = None
            
            # Get processor multiple times
            processor1 = await get_grok_processor()
            processor2 = await get_grok_processor()
            
            # Should be the same instance
            assert processor1 is processor2
            assert processor1._initialized


class TestIntegrationScenarios:
    """Test realistic integration scenarios."""
    
    @pytest.mark.asyncio
    async def test_github_repository_processing(self):
        """Test processing a typical GitHub repository crawl."""
        github_crawl = {
            "url": "https://github.com/microsoft/vscode",
            "content": """
# Visual Studio Code

Visual Studio Code is a lightweight but powerful source code editor.

## Features

- IntelliSense
- Debugging
- Git integration

## Getting Started

```javascript
// Example configuration
{
    "editor.fontSize": 14,
    "files.autoSave": "afterDelay"
}
```

## Building

```bash
npm install
npm run build
```
""",
            "content_type": "text/markdown",
            "status_code": 200,
            "depth": 0,
            "links": [
                "https://github.com/microsoft/vscode/issues",
                "https://github.com/microsoft/vscode/wiki"
            ],
            "parsed_metadata": {
                "platform": "github",
                "title": "Visual Studio Code",
                "tags": ["editor", "javascript", "typescript"]
            }
        }
        
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            processor = AdvancedGrokProcessor()
            await processor.initialize()
            
            result = await processor.process_crawl_result(github_crawl)
            
            assert result.is_success()
            data = result.unwrap()
            
            # Should have multiple chunk types
            chunk_types = data["processing_stats"]["chunk_types"]
            assert "heading" in chunk_types
            assert "code" in chunk_types
            assert "text" in chunk_types
            
            # Should detect code languages
            assert data["chunks_created"] >= 4  # At least headings + code + text
    
    @pytest.mark.asyncio 
    async def test_pypi_package_processing(self):
        """Test processing a PyPI package page."""
        pypi_crawl = {
            "url": "https://pypi.org/project/requests/",
            "content": """
# requests

**Installation:**
```bash
pip install requests
```

Python HTTP library for humans.

## Usage

```python
import requests

response = requests.get('https://api.github.com')
print(response.json())
```

## Features

- Simple API
- Session management
- SSL verification
""",
            "content_type": "text/markdown",
            "parsed_metadata": {
                "platform": "pypi",
                "package_name": "requests",
                "version": "2.31.0"
            }
        }
        
        with patch('b00t_j0b_py.grok_integration.GROK_GURU_AVAILABLE', False):
            processor = AdvancedGrokProcessor()
            await processor.initialize()
            
            result = await processor.process_crawl_result(pypi_crawl)
            
            assert result.is_success()
            data = result.unwrap()
            
            # Should preserve code blocks
            chunk_types = data["processing_stats"]["chunk_types"]
            assert "code" in chunk_types
            
            # Should apply platform tags
            assert "platform:pypi" in str(data)  # Would be in chunk metadata


if __name__ == "__main__":
    pytest.main([__file__, "-v"])