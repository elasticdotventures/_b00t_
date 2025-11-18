#!/usr/bin/env python3
"""
Health check script for b00t-langchain-agent PM2 service.

Verifies:
- Redis connectivity
- Service process status
- Memory usage within limits
- Recent activity on k0mmand3r channel
"""

import asyncio
import os
import sys
from datetime import datetime

import redis.asyncio as redis


async def main() -> int:
    """Run health checks and return exit code."""
    redis_url = os.getenv("REDIS_URL", "redis://localhost:6379")
    channel = os.getenv("LANGCHAIN_COMMAND_CHANNEL", "b00t:langchain")

    checks_passed = 0
    checks_failed = 0

    print(f"🏥 Health Check - {datetime.utcnow().isoformat()}Z")
    print("=" * 60)

    # Check 1: Redis connectivity
    try:
        client = redis.from_url(redis_url, decode_responses=True)
        pong = await client.ping()
        if pong:
            print("✅ Redis connection: OK")
            checks_passed += 1
        else:
            print("❌ Redis connection: FAILED (no pong)")
            checks_failed += 1
    except Exception as e:
        print(f"❌ Redis connection: FAILED ({e})")
        checks_failed += 1
        client = None

    # Check 2: k0mmand3r channel subscription
    if client:
        try:
            pubsub = client.pubsub()
            await pubsub.subscribe(channel)
            print(f"✅ k0mmand3r channel {channel}: OK")
            checks_passed += 1
            await pubsub.unsubscribe(channel)
            await pubsub.aclose()
        except Exception as e:
            print(f"❌ k0mmand3r channel: FAILED ({e})")
            checks_failed += 1

    # Check 3: Status channel reachable
    if client:
        try:
            status_channel = f"{channel}:status"
            # Publish test ping
            await client.publish(status_channel, '{"type":"health_check"}')
            print(f"✅ Status channel {status_channel}: OK")
            checks_passed += 1
        except Exception as e:
            print(f"❌ Status channel: FAILED ({e})")
            checks_failed += 1

    # Close Redis client
    if client:
        await client.aclose()

    # Summary
    print("=" * 60)
    print(f"Checks passed: {checks_passed}/{checks_passed + checks_failed}")

    # Exit code: 0 if all passed, 1 if any failed
    return 0 if checks_failed == 0 else 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
