"""
k0mmand3r IPC Listener for LangChain agents.

Listens to Redis pub/sub channels for slash commands and routes to AgentService.
"""

import asyncio
import json
import logging
from typing import Any

from redis.asyncio import Redis

from .agent_service import AgentService
from .job_executor import JobExecutor
from .types import AgentAction, ChainAction, K0mmand3rMessage

log = logging.getLogger(__name__)


# k0mmand3r verb mapping (per k0mmand3r_interface.md spec)
VERB_MAP = {
    "dispatch": "run",  # /k0mmand3r dispatch -> agent run
    "status": "status",  # /k0mmand3r status -> agent status
    "capability": "create",  # /k0mmand3r capability -> agent create
    "complete": "delete",  # /k0mmand3r complete -> agent delete
    "message": "broadcast",  # /k0mmand3r message -> agent broadcast
}

# Status filtering levels (per k0mmand3r_interface.md:204)
STATUS_LEVELS = {
    "critical": ["error", "fatal"],
    "compact": ["info", "error", "fatal"],
    "verbose": ["debug", "info", "warn", "error", "fatal"],
    "debug": ["trace", "debug", "info", "warn", "error", "fatal"],
}


class K0mmand3rListener:
    """Listens to k0mmand3r Redis channel and executes agent commands."""

    def __init__(
        self,
        redis_sub: Redis,
        agent_service: AgentService,
        channel: str,
        job_executor: JobExecutor | None = None,
        filter_mode: str = "compact",
    ) -> None:
        """
        Initialize k0mmand3r listener.

        Args:
            redis_sub: Redis client for subscribing (separate from pub client)
            agent_service: Agent service to execute commands
            channel: Redis channel to listen on
            job_executor: Optional job executor for job system integration
            filter_mode: Status filtering mode (critical, compact, verbose, debug)
        """
        self.redis_sub = redis_sub
        self.agent_service = agent_service
        self.channel = channel
        self.job_executor = job_executor
        self.filter_mode = filter_mode
        self.running = False

    async def start(self) -> None:
        """Start listening to Redis channel."""
        log.info(f"🎧 k0mmand3r listener starting on channel: {self.channel}")

        self.running = True

        # Subscribe to channel
        pubsub = self.redis_sub.pubsub()
        await pubsub.subscribe(self.channel)

        log.info(f"✅ Subscribed to {self.channel}")

        # Listen for messages
        try:
            async for message in pubsub.listen():
                if not self.running:
                    break

                if message["type"] != "message":
                    continue

                # Process message
                await self._handle_message(message["data"])

        except asyncio.CancelledError:
            log.info("⚠️  Listener cancelled")
        except Exception as e:
            log.error(f"❌ Listener error: {e}")
        finally:
            await pubsub.unsubscribe(self.channel)
            await pubsub.aclose()
            log.info("✅ Listener stopped")

    async def stop(self) -> None:
        """Stop listening to Redis channel."""
        log.info("🛑 Stopping k0mmand3r listener...")
        self.running = False

    async def _handle_message(self, data: str | bytes) -> None:
        """
        Handle incoming Redis message.

        Args:
            data: Message data from Redis
        """
        try:
            # Parse JSON message
            if isinstance(data, bytes):
                data = data.decode("utf-8")

            message_dict = json.loads(data)
            message = K0mmand3rMessage(**message_dict)

            log.info(f"📨 Received command: {message.verb} from {message.agent_id}")

            # Map k0mmand3r verbs to actions (per k0mmand3r_interface.md)
            if message.verb in VERB_MAP:
                # Translate k0mmand3r verb to internal action
                if "params" not in message_dict:
                    message_dict["params"] = {}
                message_dict["params"]["action"] = VERB_MAP[message.verb]
                message = K0mmand3rMessage(**message_dict)
                log.debug(f"   Mapped {message.verb} -> action={VERB_MAP[message.verb]}")

            # Route based on verb
            if message.verb in ("agent", "dispatch", "status", "capability", "complete", "message"):
                await self._handle_agent_command(message)
            elif message.verb == "chain":
                await self._handle_chain_command(message)
            else:
                log.warning(f"⚠️  Unknown verb: {message.verb}")

        except json.JSONDecodeError as e:
            log.error(f"❌ Invalid JSON message: {e}")
        except Exception as e:
            log.error(f"❌ Error handling message: {e}")

    async def _handle_agent_command(self, message: K0mmand3rMessage) -> None:
        """
        Handle /agent slash commands.

        Args:
            message: k0mmand3r message
        """
        params = message.params
        action = params.get("action", "run")

        try:
            if action in (AgentAction.RUN, AgentAction.CREATE):
                # Run agent
                agent_name = params.get("name") or params.get("agent")
                input_text = params.get("input") or message.content or ""
                model = params.get("model")

                if not agent_name:
                    log.error("❌ Missing agent name")
                    return

                result = await self.agent_service.run_agent(
                    agent_name=agent_name,
                    input_text=input_text,
                    model_override=model,
                )

                # Publish result
                await self._publish_result(result.model_dump())

            elif action == AgentAction.BROADCAST:
                # Broadcast to all agents
                broadcast_message = params.get("message") or message.content or ""
                from_agent = params.get("from")

                results = await self.agent_service.broadcast_to_agents(
                    message=broadcast_message,
                    from_agent=from_agent,
                )

                # Publish aggregated results
                await self._publish_result(
                    {
                        "action": "broadcast",
                        "results": [r.model_dump() for r in results],
                    }
                )

            elif action == AgentAction.STATUS:
                # Get agent status
                agent_name = params.get("name")
                status = {
                    "available_agents": list(self.agent_service.agent_configs.keys()),
                    "active_agents": list(self.agent_service.agents.keys()),
                }
                await self._publish_result(status)

            elif action == AgentAction.DELETE:
                # Clear cached agent
                agent_name = params.get("name")
                if agent_name:
                    self.agent_service.agents.pop(agent_name, None)
                    log.info(f"🗑️  Deleted agent: {agent_name}")

            elif action == "run-job":
                # Execute agent from job system
                if not self.job_executor:
                    log.error("❌ Job executor not initialized")
                    await self._publish_result({"error": "Job executor not available"})
                    return

                agent_type = params.get("agent_type")
                prompt = params.get("prompt", "")
                context_files = params.get("context_files", [])
                timeout_ms = params.get("timeout_ms")

                if not agent_type:
                    log.error("❌ Missing agent_type for job execution")
                    return

                result = await self.job_executor.execute_agent_task(
                    agent_type=agent_type,
                    prompt=prompt,
                    context_files=context_files,
                    timeout_ms=timeout_ms,
                )

                # Publish result
                await self._publish_result(result)

            else:
                log.warning(f"⚠️  Unknown agent action: {action}")

        except Exception as e:
            log.error(f"❌ Agent command failed: {e}")
            await self._publish_result({"error": str(e)}, level="error")

    async def _handle_chain_command(self, message: K0mmand3rMessage) -> None:
        """
        Handle /chain slash commands.

        Args:
            message: k0mmand3r message
        """
        params = message.params
        action = params.get("action", "run")

        try:
            if action == ChainAction.RUN:
                # Run chain
                chain_name = params.get("name") or params.get("chain")

                if not chain_name:
                    log.error("❌ Missing chain name")
                    return

                # Extract chain parameters (exclude action/name)
                chain_params = {
                    k: v for k, v in params.items() if k not in ("action", "name", "chain")
                }

                result = await self.agent_service.run_chain(
                    chain_name=chain_name,
                    params=chain_params,
                )

                # Publish result
                await self._publish_result(result.model_dump())

            elif action == ChainAction.STATUS:
                # Get chain status
                status = {
                    "available_chains": list(self.agent_service.chain_configs.keys()),
                }
                await self._publish_result(status)

            else:
                log.warning(f"⚠️  Unknown chain action: {action}")

        except Exception as e:
            log.error(f"❌ Chain command failed: {e}")
            await self._publish_result({"error": str(e)})

    async def _publish_result(self, result: dict[str, Any], level: str = "info") -> None:
        """
        Publish result back to Redis with k0mmand3r-compatible status format.

        Args:
            result: Result to publish
            level: Message level (trace, debug, info, warn, error, fatal)
        """
        try:
            # Check if message should be filtered
            if not self._should_publish(level):
                log.debug(f"   Filtered {level} message (filter_mode={self.filter_mode})")
                return

            # Format as k0mmand3r status message (per k0mmand3r_interface.md:215)
            from datetime import datetime

            status_message = {
                "timestamp": datetime.utcnow().isoformat() + "Z",
                "agent": f"langchain.{result.get('agent_name', 'unknown')}",
                "task_id": result.get("task_id", ""),
                "type": "error" if result.get("error") else "progress",
                "level": level,
                "message": result.get("output") or result.get("error") or result.get("message", ""),
                "metadata": result.get("metadata", {}),
            }

            # Add progress if available
            if "progress" in result.get("metadata", {}):
                status_message["progress"] = result["metadata"]["progress"]

            # Publish to status channel
            status_channel = f"{self.channel}:status"
            await self.agent_service.redis_client.publish(
                status_channel,
                json.dumps(status_message),
            )
            log.info(f"📤 Published {level} to {status_channel}")
        except Exception as e:
            log.error(f"❌ Failed to publish result: {e}")

    def _should_publish(self, level: str) -> bool:
        """
        Check if message level should be published based on filter mode.

        Args:
            level: Message level

        Returns:
            True if should publish, False if filtered
        """
        allowed_levels = STATUS_LEVELS.get(self.filter_mode, STATUS_LEVELS["compact"])
        return level in allowed_levels
