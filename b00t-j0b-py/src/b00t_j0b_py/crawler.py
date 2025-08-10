"""Core web crawler implementation with depth-based link following."""

import time
from typing import Set, List, Dict, Optional, Any
from urllib.parse import urljoin, urlparse
from returns.result import Result, Success, Failure
from returns.maybe import Maybe, Some, Nothing
import requests
from bs4 import BeautifulSoup
import markdownify

from .config import config
from .redis_client import tracker
from .robots import robots_checker, url_validator


class WebCrawler:
    """Main web crawler with depth-based traversal."""
    
    def __init__(self, 
                 max_depth: Optional[int] = None,
                 delay: Optional[float] = None,
                 user_agent: Optional[str] = None):
        """Initialize crawler with configuration."""
        self.max_depth = max_depth or config.max_depth
        self.delay = delay or config.delay
        self.user_agent = user_agent or config.user_agent
        self.session = requests.Session()
        self.session.headers.update({"User-Agent": self.user_agent})
    
    def _extract_links(self, html_content: str, base_url: str) -> Set[str]:
        """Extract all links from HTML content."""
        links = set()
        
        try:
            soup = BeautifulSoup(html_content, 'html.parser')
            
            # Extract <a> tags
            for link in soup.find_all('a', href=True):
                href = link['href']
                absolute_url = urljoin(base_url, href)
                
                # Validate and normalize
                if url_validator.is_valid_url(absolute_url):
                    normalized_url = url_validator.normalize_url(absolute_url)
                    links.add(normalized_url)
            
            # Also extract from <link> tags (for things like canonical URLs)
            for link in soup.find_all('link', href=True):
                rel = link.get('rel', [])
                if isinstance(rel, list):
                    rel = ' '.join(rel)
                
                # Only extract content-related links
                if any(r in rel for r in ['canonical', 'alternate', 'next', 'prev']):
                    href = link['href']
                    absolute_url = urljoin(base_url, href)
                    
                    if url_validator.is_valid_url(absolute_url):
                        normalized_url = url_validator.normalize_url(absolute_url)
                        links.add(normalized_url)
                        
        except Exception as e:
            # Log error but don't fail the entire crawl
            print(f"Error extracting links from {base_url}: {e}")
        
        return links
    
    def _html_to_markdown(self, html_content: str, url: str) -> str:
        """Convert HTML content to Markdown."""
        try:
            # Configure markdownify to preserve structure
            markdown = markdownify.markdownify(
                html_content,
                heading_style="ATX",  # # style headings
                bullets="-",          # Use - for bullets
                strip=['script', 'style', 'meta', 'link'],  # Remove these tags
                convert=['p', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6', 'li', 'ul', 'ol', 
                        'blockquote', 'pre', 'code', 'a', 'strong', 'em', 'table', 
                        'th', 'td', 'tr'],
                wrap=True,
                wrap_width=80
            )
            
            # Add source URL as metadata
            markdown = f"<!-- Source: {url} -->\n\n{markdown}"
            
            return markdown.strip()
            
        except Exception as e:
            # Fallback to plain text extraction
            try:
                soup = BeautifulSoup(html_content, 'html.parser')
                text = soup.get_text(separator='\n', strip=True)
                return f"<!-- Source: {url} -->\n\n{text}"
            except Exception:
                return f"<!-- Source: {url} -->\n\n[Content extraction failed]"
    
    def _fetch_url(self, url: str) -> Result[requests.Response, Exception]:
        """Fetch URL with proper error handling."""
        try:
            response = self.session.get(
                url,
                timeout=config.timeout,
                allow_redirects=True,
                stream=True  # Stream for large files
            )
            response.raise_for_status()
            
            # Check content length
            content_length = response.headers.get('content-length')
            if content_length and int(content_length) > config.max_content_size:
                return Failure(Exception(f"Content too large: {content_length} bytes"))
            
            return Success(response)
            
        except Exception as e:
            return Failure(e)
    
    def crawl_url(self, url: str, depth: int = 0) -> Result[Dict[str, Any], Exception]:
        """Crawl a single URL and return processed content."""
        # Validate URL
        if not url_validator.is_valid_url(url):
            return Failure(ValueError(f"Invalid URL: {url}"))
        
        # Normalize URL
        url = url_validator.normalize_url(url)
        
        # Check if already crawled
        if tracker.is_crawled(url):
            crawl_info = tracker.get_crawl_info(url)
            match crawl_info:
                case Some(info):
                    return Failure(Exception(f"URL already crawled at depth {info.get('depth')}"))
                case Nothing:
                    pass
        
        # Check robots.txt
        robots_result = robots_checker.is_allowed(url)
        match robots_result:
            case Success(allowed):
                if not allowed:
                    return Failure(Exception(f"Robots.txt disallows crawling {url}"))
            case Failure(error):
                return Failure(error)
        
        # Check for crawl delay
        crawl_delay = robots_checker.get_crawl_delay(url)
        match crawl_delay:
            case Some(delay):
                if delay > self.delay:
                    time.sleep(delay)
            case Nothing:
                time.sleep(self.delay)
        
        # Fetch the URL
        response_result = self._fetch_url(url)
        match response_result:
            case Success(response):
                pass
            case Failure(error):
                # Mark as crawled even if failed to avoid retry loops
                tracker.mark_crawled(url, depth, status_code=getattr(error, 'response', {}).get('status_code', 0))
                return Failure(error)
        
        try:
            # Get content
            content_type = response.headers.get('content-type', '').lower()
            
            if 'html' in content_type:
                # Process HTML content
                html_content = response.text
                markdown_content = self._html_to_markdown(html_content, url)
                
                # Extract links for further crawling
                links = set()
                if depth < self.max_depth:
                    links = self._extract_links(html_content, url)
                    # Filter to same domain if desired (configurable)
                    links = {link for link in links if url_validator.is_same_domain(url, link)}
                
                result = {
                    "url": url,
                    "depth": depth,
                    "content_type": "text/markdown",
                    "content": markdown_content,
                    "links": list(links),
                    "status_code": response.status_code,
                    "title": self._extract_title(html_content)
                }
                
            elif 'json' in content_type:
                # Handle JSON content
                json_content = response.json()
                result = {
                    "url": url,
                    "depth": depth,
                    "content_type": "application/json",
                    "content": str(json_content),  # Convert to string for storage
                    "links": [],
                    "status_code": response.status_code,
                    "title": f"JSON from {urlparse(url).path}"
                }
                
            else:
                # Handle other content types (text, etc.)
                text_content = response.text
                result = {
                    "url": url,
                    "depth": depth,
                    "content_type": content_type,
                    "content": text_content,
                    "links": [],
                    "status_code": response.status_code,
                    "title": f"Content from {urlparse(url).path}"
                }
            
            # Mark as crawled
            tracker.mark_crawled(url, depth, response.status_code)
            
            # Cache the processed content
            tracker.cache_content(url, result["content"], result["content_type"])
            
            return Success(result)
            
        except Exception as e:
            tracker.mark_crawled(url, depth, response.status_code)
            return Failure(e)
    
    def _extract_title(self, html_content: str) -> str:
        """Extract title from HTML content."""
        try:
            soup = BeautifulSoup(html_content, 'html.parser')
            title_tag = soup.find('title')
            if title_tag:
                return title_tag.get_text().strip()
            
            # Fallback to h1
            h1_tag = soup.find('h1')
            if h1_tag:
                return h1_tag.get_text().strip()
            
            return "Untitled"
            
        except Exception:
            return "Untitled"
    
    def crawl_recursive(self, start_url: str, max_depth: Optional[int] = None) -> List[Dict[str, Any]]:
        """Crawl URL recursively up to max_depth."""
        max_depth = max_depth or self.max_depth
        results = []
        
        # Start with initial URL
        to_crawl = [(start_url, 0)]
        crawled_urls = set()
        
        while to_crawl:
            url, depth = to_crawl.pop(0)
            
            if url in crawled_urls or depth > max_depth:
                continue
            
            crawled_urls.add(url)
            
            # Crawl the URL
            result = self.crawl_url(url, depth)
            match result:
                case Success(data):
                    results.append(data)
                    
                    # Add links for next depth level
                    if depth < max_depth:
                        for link in data.get("links", []):
                            if link not in crawled_urls:
                                to_crawl.append((link, depth + 1))
                                
                case Failure(error):
                    print(f"Failed to crawl {url}: {error}")
        
        return results


# Global crawler instance
crawler = WebCrawler()