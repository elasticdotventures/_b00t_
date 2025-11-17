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
from .types import AgentAction, ChainAction, K0mmand3rMessage

log = logging.getLogger(__name__)


class K0mmand3rListener:
    """Listens to k0mmand3r Redis channel and executes agent commands."""

    def __init__(
        self,
        redis_sub: Redis,
        agent_service: AgentService,
        channel: str,
    ) -> None:
        """
        Initialize k0mmand3r listener.

        Args:
            redis_sub: Redis client for subscribing (separate from pub client)
            agent_service: Agent service to execute commands
            channel: Redis channel to listen on
        """
        self.redis_sub = redis_sub
        self.agent_service = agent_service
        self.channel = channel
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

            # Route based on verb
            if message.verb == "agent":
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

            else:
                log.warning(f"⚠️  Unknown agent action: {action}")

        except Exception as e:
            log.error(f"❌ Agent command failed: {e}")
            await self._publish_result({"error": str(e)})

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

    async def _publish_result(self, result: dict[str, Any]) -> None:
        """
        Publish result back to Redis.

        Args:
            result: Result to publish
        """
        try:
            # Publish to status channel
            status_channel = f"{self.channel}:status"
            await self.agent_service.redis_client.publish(
                status_channel,
                json.dumps(result),
            )
            log.info(f"📤 Published result to {status_channel}")
        except Exception as e:
            log.error(f"❌ Failed to publish result: {e}")
