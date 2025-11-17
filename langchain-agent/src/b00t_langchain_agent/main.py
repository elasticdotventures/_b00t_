#!/usr/bin/env python3
"""
b00t LangChain Agent Service - Main entry point

Listens to Redis pub/sub channels for slash commands and executes
LangChain agents with dynamically discovered MCP tools.
"""

import asyncio
import logging
import sys
from pathlib import Path

import redis.asyncio as redis
import typer
from dotenv import load_dotenv

from .agent_service import AgentService
from .job_executor import JobExecutor
from .k0mmand3r import K0mmand3rListener
from .mcp_tools import MCPToolDiscovery

# Load environment variables
load_dotenv()

app = typer.Typer(help="b00t LangChain Agent Service")

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="[%(asctime)s] %(levelname)s - %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)
log = logging.getLogger(__name__)


async def run_service(
    redis_url: str = "redis://localhost:6379",
    channel: str = "b00t:langchain",
    datum_path: str = "~/.dotfiles/_b00t_",
) -> None:
    """Run the LangChain agent service."""
    log.info("🦜 b00t LangChain Agent Service starting...")
    log.info(f"Redis URL: {redis_url}")
    log.info(f"K0mmand3r channel: {channel}")
    log.info(f"Datum path: {datum_path}")

    # Initialize Redis clients (separate for pub and sub)
    redis_client = redis.from_url(redis_url, decode_responses=True)
    redis_sub = redis.from_url(redis_url, decode_responses=True)

    try:
        # Test connections
        await redis_client.ping()
        await redis_sub.ping()
        log.info("✅ Redis clients connected")
    except Exception as e:
        log.error(f"❌ Redis connection failed: {e}")
        sys.exit(1)

    # Initialize MCP tool discovery
    mcp_discovery = MCPToolDiscovery(datum_path=Path(datum_path).expanduser())
    try:
        await mcp_discovery.initialize()
        log.info(f"✅ Discovered {len(mcp_discovery.tools)} MCP tools")
    except Exception as e:
        log.error(f"⚠️  MCP tool discovery failed: {e}")
        log.info("Continuing without MCP tools...")

    # Initialize Agent Service
    agent_service = AgentService(
        redis_client=redis_client,
        mcp_tools=mcp_discovery.tools,
        datum_path=Path(datum_path).expanduser(),
    )
    await agent_service.initialize()

    # Initialize Job Executor for job system integration
    job_executor = JobExecutor(agent_service=agent_service)
    log.info("✅ Job executor initialized")

    # Initialize k0mmand3r listener
    listener = K0mmand3rListener(
        redis_sub=redis_sub,
        agent_service=agent_service,
        channel=channel,
        job_executor=job_executor,
    )

    # Start listening
    await listener.start()

    log.info("✅ b00t LangChain Agent Service ready")
    log.info(f"Listening for commands on channel: {channel}")

    # Keep running until interrupted
    try:
        while True:
            await asyncio.sleep(1)
    except KeyboardInterrupt:
        log.info("⚠️  SIGINT received, shutting down gracefully...")
    finally:
        await listener.stop()
        await agent_service.shutdown()
        await mcp_discovery.shutdown()
        await redis_client.aclose()
        await redis_sub.aclose()
        log.info("✅ Service stopped")


@app.command()
def serve(
    redis_url: str = typer.Option(
        "redis://localhost:6379",
        envvar="REDIS_URL",
        help="Redis connection URL",
    ),
    channel: str = typer.Option(
        "b00t:langchain",
        envvar="LANGCHAIN_COMMAND_CHANNEL",
        help="Redis channel for k0mmand3r commands",
    ),
    datum_path: str = typer.Option(
        "~/.dotfiles/_b00t_",
        envvar="_B00T_Path",
        help="Path to b00t datums",
    ),
) -> None:
    """Start the LangChain agent service."""
    asyncio.run(run_service(redis_url, channel, datum_path))


@app.command()
def test_agent(
    name: str = typer.Argument(..., help="Agent name (e.g., 'researcher')"),
    prompt: str = typer.Argument(..., help="Prompt for the agent"),
    model: str = typer.Option("anthropic/claude-sonnet-4", help="LLM model"),
    datum_path: str = typer.Option(
        "~/.dotfiles/_b00t_",
        envvar="_B00T_Path",
        help="Path to b00t datums",
    ),
) -> None:
    """Test an agent directly (without Redis)."""

    async def run_test():
        from .agent_service import AgentService

        service = AgentService(
            redis_client=None,  # type: ignore
            mcp_tools=[],
            datum_path=Path(datum_path).expanduser(),
        )
        await service.initialize()

        result = await service.run_agent(
            agent_name=name,
            input_text=prompt,
            model_override=model,
        )

        if result.success:
            typer.echo(f"✅ Agent output:\n{result.output}")
        else:
            typer.echo(f"❌ Error: {result.error}", err=True)
            raise typer.Exit(1)

    asyncio.run(run_test())


@app.command()
def list_tools(
    datum_path: str = typer.Option(
        "~/.dotfiles/_b00t_",
        help="Path to b00t datums",
    ),
) -> None:
    """List available MCP tools."""

    async def run_list():
        discovery = MCPToolDiscovery(datum_path=Path(datum_path).expanduser())
        await discovery.initialize()

        typer.echo(f"Found {len(discovery.tools)} MCP tools:\n")
        for tool in discovery.tools:
            typer.echo(f"  • {tool.name}: {tool.description}")

    asyncio.run(run_list())


if __name__ == "__main__":
    app()
