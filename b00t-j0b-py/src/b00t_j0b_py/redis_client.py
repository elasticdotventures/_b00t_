"""Redis client and utilities for tracking crawled URLs."""

import redis
import json
import hashlib
from typing import Set, Dict, Optional, Any
from returns.result import Result, Success, Failure
from returns.maybe import Maybe, Some, Nothing
from datetime import datetime, timedelta

from .config import config


class RedisTracker:
    """Redis-based URL tracking and caching system."""
    
    def __init__(self, redis_url: Optional[str] = None):
        """Initialize Redis connection."""
        url = redis_url or config.redis_url
        try:
            self.redis = redis.from_url(url, decode_responses=True)
            # Test connection
            self.redis.ping()
        except Exception as e:
            raise ConnectionError(f"Failed to connect to Redis: {e}")
    
    def _url_key(self, url: str) -> str:
        """Generate Redis key for URL tracking."""
        url_hash = hashlib.sha256(url.encode()).hexdigest()[:16]
        return f"crawl:url:{url_hash}"
    
    def _robots_key(self, domain: str) -> str:
        """Generate Redis key for robots.txt caching."""
        return f"crawl:robots:{domain}"
    
    def _content_key(self, url: str) -> str:
        """Generate Redis key for content caching."""
        url_hash = hashlib.sha256(url.encode()).hexdigest()[:16]
        return f"crawl:content:{url_hash}"
    
    def mark_crawled(self, url: str, depth: int, status_code: int = 200) -> Result[bool, Exception]:
        """Mark URL as crawled with metadata."""
        try:
            key = self._url_key(url)
            data = {
                "url": url,
                "depth": depth,
                "status_code": status_code,
                "crawled_at": datetime.utcnow().isoformat(),
            }
            # Store with 7-day expiration
            self.redis.setex(key, timedelta(days=7), json.dumps(data))
            return Success(True)
        except Exception as e:
            return Failure(e)
    
    def is_crawled(self, url: str) -> bool:
        """Check if URL has been crawled."""
        try:
            key = self._url_key(url)
            return self.redis.exists(key) > 0
        except Exception:
            return False
    
    def get_crawl_info(self, url: str) -> Maybe[Dict[str, Any]]:
        """Get crawl metadata for URL."""
        try:
            key = self._url_key(url)
            data = self.redis.get(key)
            if data:
                return Some(json.loads(data))
            return Nothing
        except Exception:
            return Nothing
    
    def cache_robots_txt(self, domain: str, robots_content: str, ttl: int = 86400) -> Result[bool, Exception]:
        """Cache robots.txt content for domain."""
        try:
            key = self._robots_key(domain)
            self.redis.setex(key, ttl, robots_content)
            return Success(True)
        except Exception as e:
            return Failure(e)
    
    def get_robots_txt(self, domain: str) -> Maybe[str]:
        """Get cached robots.txt content."""
        try:
            key = self._robots_key(domain)
            content = self.redis.get(key)
            return Some(content) if content else Nothing
        except Exception:
            return Nothing
    
    def cache_content(self, url: str, content: str, content_type: str = "text/html", ttl: int = 3600) -> Result[bool, Exception]:
        """Cache processed content."""
        try:
            key = self._content_key(url)
            data = {
                "content": content,
                "content_type": content_type,
                "cached_at": datetime.utcnow().isoformat(),
            }
            self.redis.setex(key, ttl, json.dumps(data))
            return Success(True)
        except Exception as e:
            return Failure(e)
    
    def get_cached_content(self, url: str) -> Maybe[Dict[str, Any]]:
        """Get cached content for URL."""
        try:
            key = self._content_key(url)
            data = self.redis.get(key)
            if data:
                return Some(json.loads(data))
            return Nothing
        except Exception:
            return Nothing
    
    def add_to_queue(self, urls: Set[str], depth: int, queue: str = "default") -> Result[int, Exception]:
        """Add URLs to processing queue."""
        try:
            queue_key = f"crawl:queue:{queue}"
            added = 0
            for url in urls:
                if not self.is_crawled(url):
                    item = json.dumps({"url": url, "depth": depth})
                    if self.redis.sadd(queue_key, item):
                        added += 1
            return Success(added)
        except Exception as e:
            return Failure(e)
    
    def get_queue_size(self, queue: str = "default") -> int:
        """Get size of processing queue."""
        try:
            queue_key = f"crawl:queue:{queue}"
            return self.redis.scard(queue_key)
        except Exception:
            return 0
    
    def pop_from_queue(self, queue: str = "default") -> Maybe[Dict[str, Any]]:
        """Pop URL from processing queue."""
        try:
            queue_key = f"crawl:queue:{queue}"
            item = self.redis.spop(queue_key)
            if item:
                return Some(json.loads(item))
            return Nothing
        except Exception:
            return Nothing
    
    def clear_queue(self, queue: str = "default") -> Result[bool, Exception]:
        """Clear processing queue."""
        try:
            queue_key = f"crawl:queue:{queue}"
            self.redis.delete(queue_key)
            return Success(True)
        except Exception as e:
            return Failure(e)
    
    def get_stats(self) -> Dict[str, int]:
        """Get crawling statistics."""
        try:
            stats = {}
            # Count crawled URLs
            crawled_keys = self.redis.keys("crawl:url:*")
            stats["crawled_urls"] = len(crawled_keys)
            
            # Count cached robots.txt
            robots_keys = self.redis.keys("crawl:robots:*")
            stats["cached_robots"] = len(robots_keys)
            
            # Count cached content
            content_keys = self.redis.keys("crawl:content:*")
            stats["cached_content"] = len(content_keys)
            
            # Queue sizes
            for queue in ["default", "high", "low"]:
                stats[f"{queue}_queue"] = self.get_queue_size(queue)
            
            return stats
        except Exception:
            return {}


# Global tracker instance
tracker = RedisTracker()