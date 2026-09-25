"""b00t-nats-relay — lets a Hermes session look up recent b00t hive NATS mesh
(backend/infra bus) activity on demand, via the nats_relay_recent tool.

Maintains a small in-memory ring buffer of recent messages, filled by a
background listener thread started in register(). Read-only: this plugin
never sends anything anywhere on its own — it only answers when the model
calls the tool during a turn a user initiated.
"""

import logging

from . import tools

logger = logging.getLogger(__name__)


def register(ctx):
    ctx.register_tool(
        name="nats_relay_recent",
        toolset="b00t-nats-relay",
        schema={
            "name": "nats_relay_recent",
            "description": (
                "List recent messages seen on the b00t hive NATS mesh "
                "(backend/infra bus, b00t.hive.mesh.channel.* subjects) — "
                "build events, agent status, proposals. Read-only lookup, "
                "call it when the user asks what's happening on the hive."
            ),
            "parameters": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Max messages to return (default 20)",
                    }
                },
            },
        },
        handler=tools.recent_handler,
    )
    tools.start_background_listener(logger)
