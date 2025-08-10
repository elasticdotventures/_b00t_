"""Tests for advanced chunking strategies implementation."""

import pytest
from unittest.mock import Mock, patch, AsyncMock
import asyncio
from datetime import datetime

from b00t_j0b_py.advanced_chunking import (
    StructuralChunker,
    SizeBasedChunker,
    HierarchicalChunker,
    AdvancedChunkingEngine,
    AdvancedChunk,
    ChunkMetadata,
    ChunkType,
    ChunkingStrategy,
    process_crawled_content
)


class TestStructuralChunker:
    """Test structural content analysis and chunking."""
    
    @pytest.fixture
    def chunker(self):
        """Create structural chunker instance."""
        return StructuralChunker()
    
    def test_identify_code_blocks(self, chunker):
        """Test identification of code blocks."""
        content = """
# Introduction

This is some text.

```python
def hello():
    print("Hello, world!")
```

More text here.

```rust
fn main() {
    println!("Hello, Rust!");
}
```
"""
        
        structures = chunker.identify_structures(content)
        
        # Should find 2 code blocks and 1 heading
        code_blocks = [s for s in structures if s['type'] == ChunkType.CODE]
        headings = [s for s in structures if s['type'] == ChunkType.HEADING]
        
        assert len(code_blocks) == 2
        assert len(headings) == 1
        
        # Check code block details
        python_block = next((s for s in code_blocks if s['language'] == 'python'), None)
        assert python_block is not None
        assert 'def hello():' in python_block['content']
        
        rust_block = next((s for s in code_blocks if s['language'] == 'rust'), None)
        assert rust_block is not None
        assert 'fn main()' in rust_block['content']
        
        # Check heading
        heading = headings[0]
        assert heading['level'] == 1
        assert heading['content'] == 'Introduction'
    
    def test_identify_table_structure(self, chunker):
        """Test identification of table structures."""
        content = """
# Data

| Column 1 | Column 2 | Column 3 |
|----------|----------|----------|
| Value 1  | Value 2  | Value 3  |
| Value 4  | Value 5  | Value 6  |

End of table.
"""
        
        structures = chunker.identify_structures(content)
        
        tables = [s for s in structures if s['type'] == ChunkType.TABLE]
        assert len(tables) == 1
        
        table = tables[0]
        assert '| Column 1 |' in table['content']
        assert 'Value 1' in table['content']
    
    def test_chunk_by_structure(self, chunker):
        """Test structural chunking preserves boundaries."""
        content = """
# Main Title

This is introductory text.

## Subsection

```python
def example():
    return "code"
```

Final paragraph.
"""
        
        chunks = chunker.chunk_by_structure(content)
        
        # Should have: text, heading, text, heading, code, text
        assert len(chunks) >= 4
        
        # Check that code block is preserved
        code_chunks = [c for c in chunks if c.metadata.chunk_type == ChunkType.CODE]
        assert len(code_chunks) == 1
        assert code_chunks[0].metadata.language == 'python'
        assert 'def example():' in code_chunks[0].content
        
        # Check headings
        heading_chunks = [c for c in chunks if c.metadata.chunk_type == ChunkType.HEADING]
        assert len(heading_chunks) == 2
        
        main_title = next((c for c in heading_chunks if c.metadata.heading_level == 1), None)
        assert main_title is not None
        assert 'Main Title' in main_title.content
    
    def test_chunk_metadata_enrichment(self, chunker):
        """Test that structural chunks have proper metadata."""
        content = "```javascript\nconsole.log('test');\n```"
        
        chunks = chunker.chunk_by_structure(content)
        
        assert len(chunks) == 1
        chunk = chunks[0]
        
        assert chunk.metadata.chunk_type == ChunkType.CODE
        assert chunk.metadata.language == 'javascript'
        assert 'lang:javascript' in chunk.metadata.tags
        assert chunk.metadata.strategy_used == ChunkingStrategy.STRUCTURAL


class TestSizeBasedChunker:
    """Test size-based chunking with overlap."""
    
    @pytest.fixture
    def chunker(self):
        """Create size-based chunker."""
        return SizeBasedChunker(max_chunk_size=100, overlap_size=20)
    
    def test_size_based_chunking(self, chunker):
        """Test basic size-based chunking."""
        # Create content longer than max_chunk_size
        content = " ".join([f"Word{i}" for i in range(50)])  # ~300 chars
        
        chunks = chunker.chunk_by_size(content)
        
        assert len(chunks) > 1  # Should be split
        
        # Check chunk sizes
        for chunk in chunks[:-1]:  # All but last
            assert len(chunk.content) <= 100
        
        # Check that content is preserved
        combined = "".join(chunk.content for chunk in chunks)
        assert len(combined) >= len(content) * 0.9  # Account for word boundary splitting
    
    def test_overlap_preservation(self, chunker):
        """Test that overlap provides context between chunks."""
        content = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence."
        
        chunks = chunker.chunk_by_size(content)
        
        if len(chunks) > 1:
            # Check context is added
            assert chunks[0].following_context is not None
            assert chunks[1].preceding_context is not None
            
            # Context should contain overlapping information
            assert len(chunks[0].following_context) > 0
            assert len(chunks[1].preceding_context) > 0
    
    def test_word_boundary_respect(self, chunker):
        """Test that chunking respects word boundaries."""
        content = "Complete words should not be split arbitrarily during chunking process."
        
        chunks = chunker.chunk_by_size(content)
        
        # No chunk should end or start mid-word (except possibly the last)
        for chunk in chunks[:-1]:
            assert not chunk.content.endswith(' ')  # Should end at word boundary
            assert not chunk.content.startswith(' ')  # Should start at word boundary


class TestHierarchicalChunker:
    """Test hierarchical chunk relationships."""
    
    @pytest.fixture
    def chunker(self):
        """Create hierarchical chunker."""
        return HierarchicalChunker()
    
    def test_create_hierarchy_with_headings(self, chunker):
        """Test creating hierarchy based on headings."""
        # Create chunks with heading structure
        chunks = [
            self._create_heading_chunk("Main Title", 1),
            self._create_text_chunk("Introduction text"),
            self._create_heading_chunk("Subsection", 2),
            self._create_text_chunk("Subsection content"),
            self._create_text_chunk("More content")
        ]
        
        hierarchy = chunker.create_hierarchy(chunks)
        
        # Should have 2 root chunks (2 headings)
        root_chunks = [c for c in hierarchy if c.metadata.chunk_type == ChunkType.HEADING]
        assert len(root_chunks) == 2
        
        # Main title should have children
        main_title = next((c for c in root_chunks if c.metadata.heading_level == 1), None)
        assert main_title is not None
        assert len(main_title.children) > 0
    
    def _create_heading_chunk(self, content: str, level: int) -> AdvancedChunk:
        """Helper to create heading chunk."""
        metadata = ChunkMetadata(
            chunk_id=f"heading-{level}-{hash(content)}",
            chunk_type=ChunkType.HEADING,
            heading_level=level
        )
        return AdvancedChunk(content=content, metadata=metadata)
    
    def _create_text_chunk(self, content: str) -> AdvancedChunk:
        """Helper to create text chunk."""
        metadata = ChunkMetadata(
            chunk_id=f"text-{hash(content)}",
            chunk_type=ChunkType.TEXT
        )
        return AdvancedChunk(content=content, metadata=metadata)


class TestAdvancedChunkingEngine:
    """Test the main chunking engine."""
    
    @pytest.fixture
    def engine(self):
        """Create chunking engine."""
        return AdvancedChunkingEngine()
    
    def test_strategy_selection(self, engine):
        """Test automatic strategy selection."""
        # Structural strategy for code-heavy content
        code_content = """
# API Reference

```python
def api_call():
    pass
```

```javascript
function apiCall() {}
```
"""
        
        chunks = engine.chunk_content(code_content, strategy=ChunkingStrategy.STRUCTURAL)
        
        code_chunks = [c for c in chunks if c.metadata.chunk_type == ChunkType.CODE]
        assert len(code_chunks) == 2
        
        # Check languages are detected
        languages = {c.metadata.language for c in code_chunks}
        assert 'python' in languages
        assert 'javascript' in languages
    
    def test_hybrid_strategy(self, engine):
        """Test hybrid strategy combines multiple approaches."""
        # Long content with mixed elements
        content = """
# Documentation

This is a long introduction paragraph that should be kept together because it forms a coherent thought and provides important context for understanding the rest of the document.

## Code Examples

```python
# This code block should be preserved as a unit
def complex_function():
    for i in range(100):
        print(f"Processing item {i}")
    return "done"
```

## Large Text Section

""" + " ".join([f"Sentence {i} with meaningful content." for i in range(50)])
        
        chunks = engine.chunk_content(content, strategy=ChunkingStrategy.HYBRID)
        
        # Should preserve code blocks
        code_chunks = [c for c in chunks if c.metadata.chunk_type == ChunkType.CODE]
        assert len(code_chunks) >= 1
        
        # Should have headings
        heading_chunks = [c for c in chunks if c.metadata.chunk_type == ChunkType.HEADING]
        assert len(heading_chunks) >= 2
        
        # Large text should be split if too long
        text_chunks = [c for c in chunks if c.metadata.chunk_type == ChunkType.TEXT]
        assert len(text_chunks) >= 1
    
    def test_metadata_enrichment(self, engine):
        """Test metadata enrichment from parsed content."""
        content = "# Test Content\n\nThis is test content."
        
        parsed_metadata = {
            "platform": "github",
            "title": "Test Repository",
            "tags": ["python", "testing"]
        }
        
        chunks = engine.chunk_content(content)
        enriched_chunks = engine.enrich_metadata(chunks, parsed_metadata)
        
        for chunk in enriched_chunks:
            assert "platform:github" in chunk.metadata.tags
            assert "python" in chunk.metadata.tags
            assert "testing" in chunk.metadata.tags


class TestIntegrationFunctions:
    """Test integration with crawler results."""
    
    def test_process_crawled_content(self):
        """Test processing crawler results into advanced chunks."""
        crawl_result = {
            "url": "https://github.com/test/repo",
            "content": """
# Test Repository

This is a test repository.

```python
def test():
    assert True
```

## Installation

Run `pip install test-package`.
""",
            "content_type": "text/markdown",
            "parsed_metadata": {
                "platform": "github",
                "title": "Test Repository",
                "tags": ["python"]
            }
        }
        
        chunks = process_crawled_content(crawl_result)
        
        assert len(chunks) > 0
        
        # Should have different chunk types
        chunk_types = {chunk.metadata.chunk_type for chunk in chunks}
        assert ChunkType.HEADING in chunk_types
        assert ChunkType.CODE in chunk_types
        
        # Should have platform tags
        for chunk in chunks:
            assert "platform:github" in chunk.metadata.tags
    
    def test_small_content_handling(self):
        """Test handling of very small content."""
        crawl_result = {
            "url": "https://example.com/small",
            "content": "Small content.",
            "content_type": "text/plain"
        }
        
        chunks = process_crawled_content(crawl_result)
        
        # Should still create chunks for small content
        assert len(chunks) >= 1
        assert chunks[0].metadata.source_url == "https://example.com/small"


@pytest.mark.asyncio
class TestAdvancedChunk:
    """Test AdvancedChunk functionality."""
    
    def test_chunk_creation(self):
        """Test creating advanced chunks."""
        metadata = ChunkMetadata(
            chunk_id="test-123",
            chunk_type=ChunkType.TEXT,
            source_url="https://example.com"
        )
        
        chunk = AdvancedChunk(content="Test content", metadata=metadata)
        
        assert chunk.content == "Test content"
        assert chunk.metadata.chunk_id == "test-123"
        assert chunk.metadata.chunk_type == ChunkType.TEXT
    
    def test_parent_child_relationships(self):
        """Test hierarchical relationships."""
        parent = AdvancedChunk(
            content="Parent content",
            metadata=ChunkMetadata(chunk_id="parent", chunk_type=ChunkType.HEADING)
        )
        
        child = AdvancedChunk(
            content="Child content", 
            metadata=ChunkMetadata(chunk_id="child", chunk_type=ChunkType.TEXT)
        )
        
        parent.add_child(child)
        
        assert child.parent == parent
        assert child in parent.children
        assert child.metadata.parent_id == parent.metadata.chunk_id
        assert child.metadata.chunk_id in parent.metadata.children_ids
    
    def test_to_dict_serialization(self):
        """Test chunk serialization to dictionary."""
        metadata = ChunkMetadata(
            chunk_id="test-123",
            chunk_type=ChunkType.CODE,
            language="python",
            tags=["test", "python"]
        )
        
        chunk = AdvancedChunk(content="print('test')", metadata=metadata)
        
        data = chunk.to_dict()
        
        assert data["content"] == "print('test')"
        assert data["metadata"]["chunk_id"] == "test-123"
        assert data["metadata"]["chunk_type"] == "code"
        assert data["metadata"]["language"] == "python"
        assert "test" in data["metadata"]["tags"]
        assert "python" in data["metadata"]["tags"]


if __name__ == "__main__":
    pytest.main([__file__, "-v"])