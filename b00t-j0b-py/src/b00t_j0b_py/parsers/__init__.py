"""Content parsers for different platforms and content types."""

from .base import BaseParser, ParseResult, registry
from .github_parser import GitHubParser
from .pypi_parser import PyPIParser
from .npm_parser import NPMParser
from .crates_parser import CratesParser

# Register parsers
registry.register(GitHubParser())
registry.register(PyPIParser())
registry.register(NPMParser())
registry.register(CratesParser())

__all__ = [
    "BaseParser", 
    "ParseResult",
    "registry",
    "GitHubParser",
    "PyPIParser", 
    "NPMParser",
    "CratesParser"
]