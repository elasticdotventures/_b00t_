"""Robots.txt handling and URL validation."""

import urllib.robotparser as robotparser
import validators
from typing import Optional
from urllib.parse import urljoin, urlparse
from returns.result import Result, Success, Failure
from returns.maybe import Maybe, Some, Nothing
import requests

from .config import config
from .redis_client import tracker


class RobotsChecker:
    """Handles robots.txt fetching, caching, and URL checking."""
    
    def __init__(self, user_agent: Optional[str] = None):
        """Initialize with user agent."""
        self.user_agent = user_agent or config.user_agent
    
    def _get_domain(self, url: str) -> str:
        """Extract domain from URL."""
        parsed = urlparse(url)
        return f"{parsed.scheme}://{parsed.netloc}"
    
    def _fetch_robots_txt(self, domain: str) -> Result[str, Exception]:
        """Fetch robots.txt from domain."""
        robots_url = urljoin(domain, "/robots.txt")
        
        try:
            response = requests.get(
                robots_url,
                headers={"User-Agent": self.user_agent},
                timeout=config.timeout,
                allow_redirects=True
            )
            
            # Accept 200 or 404 (no robots.txt is valid)
            if response.status_code == 200:
                return Success(response.text)
            elif response.status_code == 404:
                return Success("")  # No robots.txt = allow all
            else:
                return Failure(Exception(f"HTTP {response.status_code} for {robots_url}"))
                
        except Exception as e:
            return Failure(e)
    
    def get_robots_txt(self, url: str) -> Result[str, Exception]:
        """Get robots.txt content, using cache when possible."""
        domain = self._get_domain(url)
        
        # Try cache first
        cached_robots = tracker.get_robots_txt(domain)
        match cached_robots:
            case Some(content):
                return Success(content)
            case Nothing:
                pass
        
        # Fetch from web
        robots_result = self._fetch_robots_txt(domain)
        match robots_result:
            case Success(content):
                # Cache the result
                tracker.cache_robots_txt(domain, content)
                return Success(content)
            case Failure(error):
                return Failure(error)
    
    def is_allowed(self, url: str) -> Result[bool, Exception]:
        """Check if URL is allowed by robots.txt."""
        # First validate URL format
        if not validators.url(url):
            return Failure(ValueError(f"Invalid URL: {url}"))
        
        # Get robots.txt content
        robots_result = self.get_robots_txt(url)
        match robots_result:
            case Success(robots_content):
                pass
            case Failure(error):
                # If we can't fetch robots.txt, err on the side of caution
                return Failure(error)
        
        # Parse robots.txt
        try:
            rp = robotparser.RobotFileParser()
            rp.set_url(urljoin(self._get_domain(url), "/robots.txt"))
            if robots_content.strip():  # Only set if content exists
                # Create a mock file-like object for the content
                from io import StringIO
                rp._file = StringIO(robots_content)
                rp.read()
            else:
                # Empty robots.txt means allow all
                pass
            
            # Check if allowed for our user agent
            allowed = rp.can_fetch(self.user_agent, url)
            return Success(allowed)
            
        except Exception as e:
            return Failure(e)
    
    def get_crawl_delay(self, url: str) -> Maybe[float]:
        """Get crawl delay from robots.txt."""
        robots_result = self.get_robots_txt(url)
        match robots_result:
            case Success(robots_content):
                pass
            case Failure(_):
                return Nothing
        
        try:
            rp = robotparser.RobotFileParser()
            rp.set_url(urljoin(self._get_domain(url), "/robots.txt"))
            if robots_content.strip():  # Only set if content exists
                from io import StringIO
                rp._file = StringIO(robots_content)
                rp.read()
            else:
                return Nothing
            
            delay = rp.crawl_delay(self.user_agent)
            return Some(delay) if delay else Nothing
            
        except Exception:
            return Nothing


class URLValidator:
    """Enhanced URL validation and normalization."""
    
    @staticmethod
    def is_valid_url(url: str) -> bool:
        """Check if URL is valid and crawlable."""
        if not validators.url(url):
            return False
        
        parsed = urlparse(url)
        
        # Only HTTP/HTTPS
        if parsed.scheme not in ("http", "https"):
            return False
        
        # Must have valid domain
        if not parsed.netloc:
            return False
        
        # Skip common non-content extensions
        skip_extensions = {
            ".jpg", ".jpeg", ".png", ".gif", ".svg", ".ico",
            ".css", ".js", ".woff", ".woff2", ".ttf",
            ".mp3", ".mp4", ".avi", ".mov", ".zip", ".tar", ".gz"
        }
        
        path_lower = parsed.path.lower()
        if any(path_lower.endswith(ext) for ext in skip_extensions):
            return False
        
        return True
    
    @staticmethod
    def normalize_url(url: str) -> str:
        """Normalize URL for consistent tracking."""
        parsed = urlparse(url)
        
        # Remove fragment
        normalized = f"{parsed.scheme}://{parsed.netloc}{parsed.path}"
        
        # Add query string if present
        if parsed.query:
            normalized += f"?{parsed.query}"
        
        # Remove trailing slash unless it's root
        if normalized.endswith("/") and len(parsed.path) > 1:
            normalized = normalized[:-1]
        
        return normalized
    
    @staticmethod
    def is_same_domain(url1: str, url2: str) -> bool:
        """Check if two URLs are from the same domain."""
        try:
            domain1 = urlparse(url1).netloc
            domain2 = urlparse(url2).netloc
            return domain1 == domain2
        except Exception:
            return False


# Global instances
robots_checker = RobotsChecker()
url_validator = URLValidator()