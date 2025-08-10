"""CLI interface for b00t-j0b-py web crawler."""

import click
import validators
from typing import Optional
from rq import Queue, Worker
import redis
from returns.result import Success, Failure

from .config import config
from .jobs import crawl_url_job, digest_url_job, cleanup_old_data_job
from .redis_client import tracker
from .parsers.base import registry


@click.group()
@click.version_option("0.1.0")
def main():
    """b00t-j0b-py: Web crawler job system for b00t ecosystem."""
    pass


@main.command()
@click.argument('url')
@click.option('--depth', default=1, help='Maximum crawl depth (default: 1)')
@click.option('--queue', default='default', help='RQ queue name (default: default)')
@click.option('--sync', is_flag=True, help='Run synchronously instead of queuing')
def digest(url: str, depth: int, queue: str, sync: bool):
    """Digest a URL by crawling it and following links to specified depth."""
    
    # Validate URL
    if not validators.url(url):
        click.echo(f"❌ Invalid URL: {url}", err=True)
        return
    
    if sync:
        # Run synchronously
        click.echo(f"🕷️ Digesting {url} with depth {depth} (sync mode)")
        result = digest_url_job(url, depth)
        
        if result['status'] == 'success':
            click.echo(f"✅ Successfully crawled {result['total_pages']} pages")
            for page_result in result['results']:
                click.echo(f"  📄 {page_result['url']} ({page_result['status_code']})")
        else:
            click.echo(f"❌ Failed: {result.get('error', 'Unknown error')}", err=True)
    else:
        # Queue the job
        try:
            redis_conn = redis.from_url(config.redis_url)
            q = Queue(queue, connection=redis_conn)
            job = q.enqueue(digest_url_job, url, depth, timeout='10m')
            
            click.echo(f"🚀 Queued digest job for {url}")
            click.echo(f"   Job ID: {job.id}")
            click.echo(f"   Queue: {queue}")
            click.echo(f"   Depth: {depth}")
            
        except Exception as e:
            click.echo(f"❌ Failed to queue job: {e}", err=True)


@main.command()
@click.argument('url') 
@click.option('--depth', default=0, help='Crawl depth for this URL (default: 0)')
@click.option('--queue', default='default', help='RQ queue name (default: default)')
@click.option('--sync', is_flag=True, help='Run synchronously instead of queuing')
def crawl(url: str, depth: int, queue: str, sync: bool):
    """Crawl a single URL."""
    
    # Validate URL
    if not validators.url(url):
        click.echo(f"❌ Invalid URL: {url}", err=True)
        return
    
    if sync:
        # Run synchronously
        click.echo(f"🕷️ Crawling {url} at depth {depth} (sync mode)")
        result = crawl_url_job(url, depth)
        
        if result['status'] == 'success':
            data = result['data']
            click.echo(f"✅ Successfully crawled {url}")
            click.echo(f"   Title: {data.get('title', 'N/A')}")
            click.echo(f"   Content length: {len(data.get('content', ''))} chars")
            click.echo(f"   Links found: {len(data.get('links', []))}")
        else:
            click.echo(f"❌ Failed: {result.get('error', 'Unknown error')}", err=True)
    else:
        # Queue the job
        try:
            redis_conn = redis.from_url(config.redis_url)
            q = Queue(queue, connection=redis_conn)
            job = q.enqueue(crawl_url_job, url, depth, timeout='5m')
            
            click.echo(f"🚀 Queued crawl job for {url}")
            click.echo(f"   Job ID: {job.id}")
            click.echo(f"   Queue: {queue}")
            
        except Exception as e:
            click.echo(f"❌ Failed to queue job: {e}", err=True)


@main.command()
@click.option('--queue', multiple=True, default=['default'], help='Queue(s) to process')
@click.option('--burst', is_flag=True, help='Run in burst mode (exit when queue empty)')
def worker(queue: tuple, burst: bool):
    """Start RQ worker to process crawl jobs."""
    
    try:
        redis_conn = redis.from_url(config.redis_url)
        queues = [Queue(q, connection=redis_conn) for q in queue]
        
        click.echo(f"🔧 Starting worker for queues: {', '.join(queue)}")
        click.echo(f"   Redis: {config.redis_url}")
        click.echo(f"   Burst mode: {burst}")
        
        worker = Worker(queues, connection=redis_conn)
        worker.work(with_scheduler=True, burst=burst)
        
    except KeyboardInterrupt:
        click.echo("\n👋 Worker stopped by user")
    except Exception as e:
        click.echo(f"❌ Worker error: {e}", err=True)


@main.command()
@click.option('--queue', default='default', help='Queue name to check')
def status(queue: str):
    """Show crawler status and statistics."""
    
    try:
        # Get Redis stats
        stats = tracker.get_stats()
        
        click.echo("📊 b00t-j0b-py Crawler Status")
        click.echo("=" * 40)
        click.echo(f"URLs crawled: {stats.get('crawled_urls', 0)}")
        click.echo(f"Robots.txt cached: {stats.get('cached_robots', 0)}")
        click.echo(f"Content cached: {stats.get('cached_content', 0)}")
        click.echo()
        
        click.echo("Queue sizes:")
        for queue_name in ['default', 'high', 'low']:
            size = stats.get(f"{queue_name}_queue", 0)
            click.echo(f"  {queue_name}: {size} jobs")
        
        # Test Redis connection
        try:
            redis_conn = redis.from_url(config.redis_url)
            redis_conn.ping()
            click.echo(f"\n✅ Redis connection OK ({config.redis_url})")
        except Exception as e:
            click.echo(f"\n❌ Redis connection failed: {e}")
            
    except Exception as e:
        click.echo(f"❌ Failed to get status: {e}", err=True)


@main.command()
@click.option('--queue', default='default', help='Queue to clear')
@click.confirmation_option(prompt='Are you sure you want to clear the queue?')
def clear_queue(queue: str):
    """Clear all jobs from specified queue."""
    
    try:
        result = tracker.clear_queue(queue)
        match result:
            case Success(_):
                click.echo(f"✅ Cleared queue '{queue}'")
            case Failure(error):
                click.echo(f"❌ Failed to clear queue: {error}", err=True)
    except Exception as e:
        click.echo(f"❌ Error: {e}", err=True)


@main.command()
@click.option('--sync', is_flag=True, help='Run synchronously instead of queuing')
def cleanup(sync: bool):
    """Clean up old crawl data."""
    
    if sync:
        click.echo("🧹 Running cleanup (sync mode)")
        result = cleanup_old_data_job()
        
        if result['status'] == 'success':
            click.echo("✅ Cleanup completed")
        else:
            click.echo(f"❌ Cleanup failed: {result.get('error', 'Unknown error')}", err=True)
    else:
        try:
            redis_conn = redis.from_url(config.redis_url)
            q = Queue('low', connection=redis_conn)  # Low priority queue
            job = q.enqueue(cleanup_old_data_job, timeout='30m')
            
            click.echo(f"🚀 Queued cleanup job")
            click.echo(f"   Job ID: {job.id}")
            
        except Exception as e:
            click.echo(f"❌ Failed to queue cleanup: {e}", err=True)


@main.command()
def parsers():
    """List available content parsers."""
    
    click.echo("🔍 Available Content Parsers")
    click.echo("=" * 40)
    
    test_urls = [
        ("https://github.com/user/repo", "GitHub"),
        ("https://pypi.org/project/requests", "PyPI"),
        ("https://npmjs.com/package/react", "NPM"),
        ("https://crates.io/crates/serde", "Crates.io")
    ]
    
    for url, platform in test_urls:
        parser = registry.get_parser(url)
        if parser:
            click.echo(f"✅ {platform}: {parser.__class__.__name__}")
        else:
            click.echo(f"❌ {platform}: No parser available")


if __name__ == '__main__':
    main()