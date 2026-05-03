"""RQ integration utilities and worker setup."""

import os
from typing import List, Optional
from rq import Queue, Worker, Connection
import redis
from rq.job import Job

from .config import config
from .parsers.base import registry
from .parsers import GitHubParser, PyPIParser, NPMParser, CratesParser


def setup_parsers():
    """Register all parsers with the global registry."""
    registry.register(GitHubParser())
    registry.register(PyPIParser())
    registry.register(NPMParser())
    registry.register(CratesParser())


def get_redis_connection(redis_url: Optional[str] = None) -> redis.Redis:
    """Get Redis connection."""
    url = redis_url or config.redis_url
    return redis.from_url(url, decode_responses=True)


def get_queue(queue_name: str = "default", redis_url: Optional[str] = None) -> Queue:
    """Get RQ queue instance."""
    conn = get_redis_connection(redis_url)
    return Queue(queue_name, connection=conn)


def get_all_queues(redis_url: Optional[str] = None) -> List[Queue]:
    """Get all configured queues."""
    conn = get_redis_connection(redis_url)
    return [
        Queue(config.default_queue, connection=conn),
        Queue(config.high_queue, connection=conn),
        Queue(config.low_queue, connection=conn),
    ]


def start_worker(queue_names: Optional[List[str]] = None, 
                 burst: bool = False,
                 redis_url: Optional[str] = None) -> Worker:
    """Start RQ worker for specified queues."""
    # Setup parsers
    setup_parsers()
    
    # Get connection
    conn = get_redis_connection(redis_url)
    
    # Setup queues
    if not queue_names:
        queue_names = [config.default_queue]
    
    queues = [Queue(name, connection=conn) for name in queue_names]
    
    # Create and start worker
    worker = Worker(queues, connection=conn)
    
    if burst:
        worker.work(burst=True, with_scheduler=True)
    else:
        worker.work(with_scheduler=True)
    
    return worker


def get_job_status(job_id: str, redis_url: Optional[str] = None) -> dict:
    """Get status of a specific job."""
    conn = get_redis_connection(redis_url)
    
    try:
        job = Job.fetch(job_id, connection=conn)
        
        return {
            "id": job.id,
            "status": job.get_status(),
            "created_at": job.created_at.isoformat() if job.created_at else None,
            "started_at": job.started_at.isoformat() if job.started_at else None,
            "ended_at": job.ended_at.isoformat() if job.ended_at else None,
            "result": job.result,
            "exc_info": job.exc_info,
            "meta": job.meta,
            "func_name": job.func_name,
            "args": job.args,
            "kwargs": job.kwargs,
        }
    except Exception as e:
        return {
            "id": job_id,
            "status": "not_found",
            "error": str(e)
        }


def get_queue_info(queue_name: str = "default", redis_url: Optional[str] = None) -> dict:
    """Get information about a queue."""
    queue = get_queue(queue_name, redis_url)
    
    return {
        "name": queue.name,
        "length": len(queue),
        "is_empty": queue.is_empty(),
        "job_ids": queue.job_ids,
        "started_job_registry": list(queue.started_job_registry.get_job_ids()),
        "finished_job_registry": list(queue.finished_job_registry.get_job_ids()),
        "failed_job_registry": list(queue.failed_job_registry.get_job_ids()),
        "deferred_job_registry": list(queue.deferred_job_registry.get_job_ids()),
    }


def clear_all_queues(redis_url: Optional[str] = None) -> dict:
    """Clear all jobs from all queues."""
    conn = get_redis_connection(redis_url)
    
    results = {}
    for queue_name in [config.default_queue, config.high_queue, config.low_queue]:
        queue = Queue(queue_name, connection=conn)
        count = len(queue)
        queue.empty()
        results[queue_name] = count
    
    return results


# Worker startup hook
def setup_worker_environment():
    """Setup worker environment - called when worker starts."""
    # Setup parsers
    setup_parsers()
    
    # Set environment variables if needed
    os.environ.setdefault("RQ_WORKER", "true")
    
    # Any other worker initialization can go here
    print("🔧 b00t-j0b-py worker environment initialized")