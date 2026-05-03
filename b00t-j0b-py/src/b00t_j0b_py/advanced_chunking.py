"""Advanced chunking strategies for Phase 3a.2 implementation.

Implements multi-strategy chunking with:
- Semantic chunking via b00t-grok integration  
- Structural chunking (code blocks, tables, lists)
- Size-based chunking with overlap
- Hierarchical parent-child relationships
- Metadata enrichment (headings, language tags, etc.)
"""

import re
import hashlib
from typing import List, Dict, Any, Optional, Tuple, Union
from dataclasses import dataclass, field
from enum import Enum
import uuid
from datetime import datetime

try:
    # Try to import b00t-grok-py for integration
    from b00t_grok_guru import GrokGuru
    GROK_AVAILABLE = True
except ImportError:
    GROK_AVAILABLE = False
    # Create a mock type for type hints when not available
    class GrokGuru:
        pass


class ChunkType(Enum):
    """Types of content chunks."""
    TEXT = "text"
    CODE = "code" 
    TABLE = "table"
    LIST = "list"
    HEADING = "heading"
    METADATA = "metadata"
    COMPOSITE = "composite"


class ChunkingStrategy(Enum):
    """Available chunking strategies."""
    SEMANTIC = "semantic"      # Meaning-based boundaries via ML
    STRUCTURAL = "structural"  # Markdown/HTML structure aware  
    SIZE_BASED = "size_based"  # Fixed size with overlap
    HYBRID = "hybrid"          # Combination of strategies


@dataclass
class ChunkMetadata:
    """Enhanced metadata for chunks."""
    # Basic identification
    chunk_id: str
    parent_id: Optional[str] = None
    children_ids: List[str] = field(default_factory=list)
    
    # Content classification  
    chunk_type: ChunkType = ChunkType.TEXT
    language: Optional[str] = None  # Programming language for code
    heading_level: Optional[int] = None  # H1-H6 level
    
    # Source attribution
    source_url: Optional[str] = None
    source_file: Optional[str] = None
    xpath: Optional[str] = None  # Path within document structure
    
    # Processing metadata
    strategy_used: ChunkingStrategy = ChunkingStrategy.SEMANTIC
    chunk_index: int = 0
    total_chunks: int = 0
    
    # Content characteristics
    char_count: int = 0
    word_count: int = 0
    line_count: int = 0
    
    # Timestamps
    created_at: datetime = field(default_factory=datetime.utcnow)
    
    # Tags and categories
    tags: List[str] = field(default_factory=list)
    categories: List[str] = field(default_factory=list)


@dataclass  
class AdvancedChunk:
    """Enhanced chunk with hierarchical relationships and metadata."""
    content: str
    metadata: ChunkMetadata
    
    # Hierarchical relationships  
    parent: Optional['AdvancedChunk'] = None
    children: List['AdvancedChunk'] = field(default_factory=list)
    
    # Context preservation
    preceding_context: Optional[str] = None  # Previous chunk preview
    following_context: Optional[str] = None  # Next chunk preview
    
    def add_child(self, child: 'AdvancedChunk') -> None:
        """Add a child chunk."""
        child.parent = self
        child.metadata.parent_id = self.metadata.chunk_id
        self.children.append(child)
        self.metadata.children_ids.append(child.metadata.chunk_id)
    
    def to_dict(self) -> Dict[str, Any]:
        """Convert to dictionary for storage/transmission."""
        return {
            "content": self.content,
            "metadata": {
                "chunk_id": self.metadata.chunk_id,
                "parent_id": self.metadata.parent_id,
                "children_ids": self.metadata.children_ids,
                "chunk_type": self.metadata.chunk_type.value,
                "language": self.metadata.language,
                "heading_level": self.metadata.heading_level,
                "source_url": self.metadata.source_url,
                "source_file": self.metadata.source_file,
                "xpath": self.metadata.xpath,
                "strategy_used": self.metadata.strategy_used.value,
                "chunk_index": self.metadata.chunk_index,
                "total_chunks": self.metadata.total_chunks,
                "char_count": self.metadata.char_count,
                "word_count": self.metadata.word_count,
                "line_count": self.metadata.line_count,
                "created_at": self.metadata.created_at.isoformat(),
                "tags": self.metadata.tags,
                "categories": self.metadata.categories
            },
            "preceding_context": self.preceding_context,
            "following_context": self.following_context
        }


class StructuralChunker:
    """Markdown/HTML structure-aware chunker."""
    
    def __init__(self):
        # Patterns for different structural elements
        self.code_block_pattern = re.compile(r'```(\w+)?\s*(.*?)\s*```', re.DOTALL)
        self.heading_pattern = re.compile(r'^(#{1,6})\s+(.+)$', re.MULTILINE)
        self.table_pattern = re.compile(r'^\|.*\|$', re.MULTILINE)
        self.list_pattern = re.compile(r'^[\s]*[-*+]\s+', re.MULTILINE)
        self.numbered_list_pattern = re.compile(r'^[\s]*\d+\.\s+', re.MULTILINE)
        
    def identify_structures(self, content: str) -> List[Dict[str, Any]]:
        """Identify structural elements in content."""
        structures = []
        
        # Find code blocks
        for match in self.code_block_pattern.finditer(content):
            language = match.group(1) or 'unknown'
            code_content = match.group(2).strip()
            structures.append({
                'type': ChunkType.CODE,
                'start': match.start(),
                'end': match.end(),
                'language': language,
                'content': code_content
            })
        
        # Find headings  
        for match in self.heading_pattern.finditer(content):
            level = len(match.group(1))
            structures.append({
                'type': ChunkType.HEADING,
                'start': match.start(),
                'end': match.end(),
                'level': level,
                'content': match.group(2),
                'heading_level': level
            })
        
        # Find tables (simple detection)
        table_lines = []
        for match in self.table_pattern.finditer(content):
            table_lines.append((match.start(), match.end(), match.group()))
        
        if table_lines:
            # Group consecutive table lines
            start = table_lines[0][0]
            end = table_lines[-1][1]
            table_content = content[start:end]
            structures.append({
                'type': ChunkType.TABLE,
                'start': start,
                'end': end,
                'content': table_content
            })
        
        # Sort by position
        structures.sort(key=lambda x: x['start'])
        return structures
    
    def chunk_by_structure(self, content: str, source_url: Optional[str] = None) -> List[AdvancedChunk]:
        """Chunk content preserving structural boundaries."""
        structures = self.identify_structures(content)
        chunks = []
        last_end = 0
        
        for i, struct in enumerate(structures):
            # Add text content before this structure
            if struct['start'] > last_end:
                text_content = content[last_end:struct['start']].strip()
                if text_content:
                    chunks.append(self._create_text_chunk(
                        text_content, i, len(structures), source_url
                    ))
            
            # Add the structural element as its own chunk
            chunks.append(self._create_structural_chunk(
                struct, i, len(structures), source_url
            ))
            
            last_end = struct['end']
        
        # Add remaining text content
        if last_end < len(content):
            text_content = content[last_end:].strip()
            if text_content:
                chunks.append(self._create_text_chunk(
                    text_content, len(structures), len(structures) + 1, source_url
                ))
        
        return chunks
    
    def _create_text_chunk(self, content: str, index: int, total: int, source_url: Optional[str]) -> AdvancedChunk:
        """Create a text chunk with metadata."""
        chunk_id = str(uuid.uuid4())
        
        metadata = ChunkMetadata(
            chunk_id=chunk_id,
            chunk_type=ChunkType.TEXT,
            strategy_used=ChunkingStrategy.STRUCTURAL,
            chunk_index=index,
            total_chunks=total,
            source_url=source_url,
            char_count=len(content),
            word_count=len(content.split()),
            line_count=content.count('\n') + 1
        )
        
        return AdvancedChunk(content=content, metadata=metadata)
    
    def _create_structural_chunk(self, struct: Dict[str, Any], index: int, total: int, source_url: Optional[str]) -> AdvancedChunk:
        """Create a chunk for structural elements."""
        chunk_id = str(uuid.uuid4())
        
        metadata = ChunkMetadata(
            chunk_id=chunk_id,
            chunk_type=struct['type'],
            language=struct.get('language'),
            heading_level=struct.get('heading_level'),
            strategy_used=ChunkingStrategy.STRUCTURAL,
            chunk_index=index,
            total_chunks=total,
            source_url=source_url,
            char_count=len(struct['content']),
            word_count=len(struct['content'].split()),
            line_count=struct['content'].count('\n') + 1
        )
        
        # Add appropriate tags
        if struct['type'] == ChunkType.CODE and struct.get('language'):
            metadata.tags.append(f"lang:{struct['language']}")
        elif struct['type'] == ChunkType.HEADING:
            metadata.tags.append(f"heading:h{struct['heading_level']}")
        
        return AdvancedChunk(content=struct['content'], metadata=metadata)


class SizeBasedChunker:
    """Size-based chunker with overlap for context preservation."""
    
    def __init__(self, max_chunk_size: int = 1000, overlap_size: int = 200):
        self.max_chunk_size = max_chunk_size
        self.overlap_size = overlap_size
    
    def chunk_by_size(self, content: str, source_url: Optional[str] = None) -> List[AdvancedChunk]:
        """Chunk content by size with overlap."""
        chunks = []
        start = 0
        index = 0
        
        while start < len(content):
            # Calculate chunk end
            end = min(start + self.max_chunk_size, len(content))
            
            # Try to break at word boundary if not at end
            if end < len(content):
                # Find last space before the limit
                last_space = content.rfind(' ', start, end)
                if last_space > start:
                    end = last_space
            
            chunk_content = content[start:end].strip()
            if chunk_content:
                chunk_id = str(uuid.uuid4())
                
                metadata = ChunkMetadata(
                    chunk_id=chunk_id,
                    chunk_type=ChunkType.TEXT,
                    strategy_used=ChunkingStrategy.SIZE_BASED,
                    chunk_index=index,
                    source_url=source_url,
                    char_count=len(chunk_content),
                    word_count=len(chunk_content.split()),
                    line_count=chunk_content.count('\n') + 1
                )
                
                chunk = AdvancedChunk(content=chunk_content, metadata=metadata)
                
                # Add context from previous and next chunks
                if index > 0:
                    prev_start = max(0, start - self.overlap_size)
                    chunk.preceding_context = content[prev_start:start][-100:]  # Last 100 chars
                
                if end < len(content):
                    next_end = min(len(content), end + self.overlap_size)
                    chunk.following_context = content[end:next_end][:100]  # First 100 chars
                
                chunks.append(chunk)
                index += 1
            
            # Move start position with overlap
            start = end - self.overlap_size if end < len(content) else end
        
        # Update total_chunks for all chunks
        for chunk in chunks:
            chunk.metadata.total_chunks = len(chunks)
        
        return chunks


class HierarchicalChunker:
    """Creates hierarchical chunk relationships."""
    
    def create_hierarchy(self, chunks: List[AdvancedChunk]) -> List[AdvancedChunk]:
        """Build hierarchical relationships between chunks."""
        if not chunks:
            return chunks
        
        # Group chunks by headings to create hierarchy
        current_section = None
        
        for chunk in chunks:
            if chunk.metadata.chunk_type == ChunkType.HEADING:
                # This starts a new section
                current_section = chunk
            elif current_section is not None:
                # Add as child to current section
                current_section.add_child(chunk)
        
        # Return all chunks (relationships are now established)
        return chunks


class AdvancedChunkingEngine:
    """Main engine that combines all chunking strategies."""
    
    def __init__(self, 
                 grok_guru: Optional[GrokGuru] = None,
                 default_strategy: ChunkingStrategy = ChunkingStrategy.HYBRID):
        self.grok_guru = grok_guru
        self.default_strategy = default_strategy
        
        # Initialize component chunkers
        self.structural_chunker = StructuralChunker()
        self.size_based_chunker = SizeBasedChunker()
        self.hierarchical_chunker = HierarchicalChunker()
    
    def chunk_content(self, 
                     content: str,
                     source_url: Optional[str] = None,
                     strategy: Optional[ChunkingStrategy] = None) -> List[AdvancedChunk]:
        """Main chunking method using specified or default strategy."""
        strategy = strategy or self.default_strategy
        
        if strategy == ChunkingStrategy.STRUCTURAL:
            return self._chunk_structural(content, source_url)
        elif strategy == ChunkingStrategy.SIZE_BASED:
            return self._chunk_size_based(content, source_url)
        elif strategy == ChunkingStrategy.SEMANTIC:
            return self._chunk_semantic(content, source_url)
        elif strategy == ChunkingStrategy.HYBRID:
            return self._chunk_hybrid(content, source_url)
        else:
            raise ValueError(f"Unknown chunking strategy: {strategy}")
    
    def _chunk_structural(self, content: str, source_url: Optional[str]) -> List[AdvancedChunk]:
        """Chunk using structural analysis."""
        chunks = self.structural_chunker.chunk_by_structure(content, source_url)
        return self.hierarchical_chunker.create_hierarchy(chunks)
    
    def _chunk_size_based(self, content: str, source_url: Optional[str]) -> List[AdvancedChunk]:
        """Chunk using size-based strategy."""
        return self.size_based_chunker.chunk_by_size(content, source_url)
    
    def _chunk_semantic(self, content: str, source_url: Optional[str]) -> List[AdvancedChunk]:
        """Chunk using semantic analysis via b00t-grok."""
        if not GROK_AVAILABLE or not self.grok_guru:
            # Fallback to structural chunking
            return self._chunk_structural(content, source_url)
        
        # Use b00t-grok's semantic chunking
        # This would integrate with the existing learn() method
        # For now, implement a placeholder that calls structural + size hybrid
        return self._chunk_hybrid(content, source_url)
    
    def _chunk_hybrid(self, content: str, source_url: Optional[str]) -> List[AdvancedChunk]:
        """Combine multiple strategies for optimal chunking."""
        # Start with structural analysis to identify special elements
        structural_chunks = self.structural_chunker.chunk_by_structure(content, source_url)
        
        # For large text chunks, apply size-based splitting
        refined_chunks = []
        for chunk in structural_chunks:
            if (chunk.metadata.chunk_type == ChunkType.TEXT and 
                len(chunk.content) > 1500):  # Large text chunks need further splitting
                
                # Apply size-based chunking to large text
                sub_chunks = self.size_based_chunker.chunk_by_size(chunk.content, source_url)
                for sub_chunk in sub_chunks:
                    sub_chunk.metadata.strategy_used = ChunkingStrategy.HYBRID
                    sub_chunk.metadata.tags.append("hybrid:text-split")
                refined_chunks.extend(sub_chunks)
            else:
                chunk.metadata.strategy_used = ChunkingStrategy.HYBRID
                refined_chunks.append(chunk)
        
        # Apply hierarchical organization
        return self.hierarchical_chunker.create_hierarchy(refined_chunks)
    
    def enrich_metadata(self, chunks: List[AdvancedChunk], parsed_metadata: Optional[Dict[str, Any]] = None) -> List[AdvancedChunk]:
        """Enrich chunks with additional metadata from parsed content."""
        if not parsed_metadata:
            return chunks
        
        # Add platform-specific tags and categories
        platform = parsed_metadata.get("platform", "unknown")
        title = parsed_metadata.get("title", "")
        tags = parsed_metadata.get("tags", [])
        
        for chunk in chunks:
            chunk.metadata.tags.extend([f"platform:{platform}"])
            chunk.metadata.tags.extend(tags)
            
            if title:
                chunk.metadata.tags.append(f"source:{title[:50]}")
            
            # Add content-type specific enrichment
            if chunk.metadata.chunk_type == ChunkType.CODE:
                chunk.metadata.categories.append("code")
                if chunk.metadata.language:
                    chunk.metadata.categories.append(f"lang-{chunk.metadata.language}")
            elif chunk.metadata.chunk_type == ChunkType.HEADING:
                chunk.metadata.categories.append("structure")
                chunk.metadata.categories.append(f"heading-h{chunk.metadata.heading_level}")
        
        return chunks


# Integration function for b00t-j0b-py
def process_crawled_content(crawl_result: Dict[str, Any], 
                          grok_guru: Optional[GrokGuru] = None) -> List[AdvancedChunk]:
    """Process crawled content from b00t-j0b-py into advanced chunks."""
    engine = AdvancedChunkingEngine(grok_guru)
    
    content = crawl_result.get("content", "")
    source_url = crawl_result.get("url", "")
    parsed_metadata = crawl_result.get("parsed_metadata", {})
    
    # Determine chunking strategy based on content type
    strategy = ChunkingStrategy.HYBRID
    
    # Override strategy for specific content types
    if crawl_result.get("content_type") == "code":
        strategy = ChunkingStrategy.STRUCTURAL
    elif len(content) < 500:  # Small content
        strategy = ChunkingStrategy.STRUCTURAL
    
    # Perform chunking
    chunks = engine.chunk_content(content, source_url, strategy)
    
    # Enrich with crawler metadata
    chunks = engine.enrich_metadata(chunks, parsed_metadata)
    
    return chunks