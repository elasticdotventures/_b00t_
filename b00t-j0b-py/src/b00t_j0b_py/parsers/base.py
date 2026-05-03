"""Base parser interface for different content types."""

from abc import ABC, abstractmethod
from typing import Dict, Any, Optional, List
from pydantic import BaseModel
from urllib.parse import urlparse
from returns.result import Result, Success, Failure


class ParseResult(BaseModel):
    """Standardized parse result."""
    
    url: str
    title: str
    content: str
    content_type: str
    metadata: Dict[str, Any] = {}
    links: List[str] = []
    tags: List[str] = []
    
    class Config:
        extra = "allow"


class BaseParser(ABC):
    """Abstract base parser for different content types."""
    
    def __init__(self):
        """Initialize parser."""
        pass
    
    @abstractmethod
    def can_parse(self, url: str) -> bool:
        """Check if this parser can handle the given URL."""
        pass
    
    @abstractmethod
    def parse(self, url: str, content: str, content_type: str) -> Result[ParseResult, Exception]:
        """Parse content from URL."""
        pass
    
    def get_domain(self, url: str) -> str:
        """Extract domain from URL."""
        return urlparse(url).netloc
    
    def get_path_segments(self, url: str) -> List[str]:
        """Get URL path segments."""
        path = urlparse(url).path
        return [seg for seg in path.split('/') if seg]


class ParserRegistry:
    """Registry for content parsers."""
    
    def __init__(self):
        """Initialize parser registry."""
        self._parsers: List[BaseParser] = []
    
    def register(self, parser: BaseParser) -> None:
        """Register a parser."""
        self._parsers.append(parser)
    
    def get_parser(self, url: str) -> Optional[BaseParser]:
        """Get appropriate parser for URL."""
        for parser in self._parsers:
            if parser.can_parse(url):
                return parser
        return None
    
    def parse_content(self, url: str, content: str, content_type: str) -> Result[ParseResult, Exception]:
        """Parse content using appropriate parser."""
        parser = self.get_parser(url)
        if not parser:
            return Failure(ValueError(f"No parser available for URL: {url}"))
        
        return parser.parse(url, content, content_type)


# Global registry instance
registry = ParserRegistry()