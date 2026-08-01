"""b00t-py: High-performance Python bindings for b00t-cli

Provides native Rust performance for b00t ecosystem operations:
  - EmojiRegistry: compile-time emoji lookup (shortcode, g0spell, literal)
  - check_guards: evaluate commands against guard rules
  - parse_kmdline: parse k0mmand3r slash command syntax
  - MCP, model, and datum management APIs
"""

from typing import List, Dict, Any, Optional, Union
import json
import importlib

# Lazy-load the native _core module on first access to avoid circular import
# between b00t_py (the package __init__.py) and b00t_py._core (the native .so).
_core = None

def _get_core():
    global _core
    if _core is not None:
        return _core
    try:
        _core = importlib.import_module("b00t_py._core")
    except ImportError:
        _core = None
    return _core


def _version() -> str:
    c = _get_core()
    return c.version() if c else "dev"


from .exceptions import B00tError

__version__ = _version()


# ── EmojiRegistry ──────────────────────────────────────────────────────────────

class EmojiRegistry:
    """Compile-time emoji registry with shortcode/g0spell/literal lookup.

    Wraps the native Rust EmojiRegistry pyclass for Pythonic access patterns.

    Examples:
        >>> reg = EmojiRegistry()
        >>> skunk = reg.lookup_shortcode(":skunk:")
        >>> assert skunk == "🦨"
        >>> poop = reg.lookup_g0spell("antipattern")
        >>> assert poop == "💩"
    """

    def __init__(self):
        c = _get_core()
        if c is None:
            raise B00tError("Native b00t_py module not available")
        self._inner = c.EmojiRegistry()

    def lookup_shortcode(self, shortcode: str) -> Optional[str]:
        """Look up emoji by colon-wrapped shortcode (e.g. ':skunk:').

        Returns the literal emoji string (e.g. '🦨') or None.
        """
        return self._inner.lookup_shortcode(shortcode)

    def lookup_g0spell(self, g0spell: str) -> Optional[str]:
        """Look up emoji by g0spell key (e.g. 'skunk').

        Returns the literal emoji string (e.g. '🦨') or None.
        """
        return self._inner.lookup_g0spell(g0spell)

    def lookup_literal(self, literal: str) -> Optional[Dict[str, Any]]:
        """Look up emoji by its Unicode literal (e.g. '🦨').

        Returns an entry dict with keys: literal, shortcode, g0spell,
        tier, action, description.
        """
        return self._inner.lookup_literal(literal)

    def filter_tier(self, tier: int) -> List[Dict[str, Any]]:
        """Return all entries at the given tier (0=pass, 1=warn, 2=block)."""
        return self._inner.filter_tier(tier)

    @property
    def entries(self) -> List[Dict[str, Any]]:
        """All registry entries as dicts."""
        return self._inner.entries()

    def __len__(self) -> int:
        return len(self._inner)

    def __repr__(self) -> str:
        return repr(self._inner)


# ── Guard system ───────────────────────────────────────────────────────────────

def check_guards(
    command: str,
    guards: Optional[List[Dict[str, Any]]] = None,
    context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """Evaluate a command string against a list of guard rules.

    Args:
        command: The command string to check (e.g. "pip install flask").
        guards: List of guard definition dicts. If None, uses a default
            guard that warns on "pip install".
        context: Optional dict with keys:
            - "violation_count" (int): Current count for 🦨→💩 escalation
            - "rhai_macros" (dict): Rhai macro definitions

    Each guard dict supports:
        - "pattern" (str or dict): If string, matched as substring.
            If dict: {"rhai": "expr"} or {"stage": "name"}.
        - "action" (str): "warn", "block", or "redirect"
        - "message" (str, optional): Custom warning/block message
        - "redirect" (str, optional): Redirect command suggestion
        - "repeat_threshold" (int, optional): Repeat count for 🦨→💩 escalation

    Returns:
        dict with keys: action ("allow"|"warn"|"block"), message, redirect (optional)

    Examples:
        >>> result = check_guards("pip install flask")
        >>> assert result["action"] == "warn"
        >>> assert "uv pip install" in result["message"]

        >>> guards = [{"pattern": "rm -rf /", "action": "block", "message": "🚫 BLOCKED"}]
        >>> result = check_guards("rm -rf /", guards)
        >>> assert result["action"] == "block"
    """
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available. Install with: pip install b00t-py")
    return json.loads(c.check_guards_py(command, guards, context))


# ── Guard system (new API) ────────────────────────────────────────────────────

def guard_check(command: str, guards_json: Optional[str] = None) -> Dict[str, Any]:
    """Evaluate a command string against guard rules (new API).

    Args:
        command: The command string to check.
        guards_json: Optional JSON string of guard definitions.

    Returns:
        dict with keys: action ("allow"|"warn"|"block"), message, redirect (optional)
    """
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return json.loads(c.guard_check(command, guards_json))


def guard_violations() -> Dict[str, int]:
    """Get current violation counts for all guard patterns."""
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return json.loads(c.guard_violations())


def guard_reset(pattern_key: str) -> str:
    """Reset violation count for a guard pattern."""
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return c.guard_reset(pattern_key)


def guard_coverage() -> str:
    """Run guard coverage scan description."""
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return c.guard_coverage()


# ── Emoji registry ────────────────────────────────────────────────────────────

def emoji_lookup(key: str) -> Dict[str, Any]:
    """Look up an emoji by shortcode, g0spell, or literal.

    Returns dict with keys: found, literal, shortcode, g0spell, tier, action, description.
    If not found, returns {"found": false, "key": key}.
    """
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return json.loads(c.emoji_lookup(key))


def emoji_list() -> List[Dict[str, Any]]:
    """List all emoji registry entries."""
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return json.loads(c.emoji_list())


# ── Parser stage guards ───────────────────────────────────────────────────────

def register_stage_guard(stage: str, callback) -> Dict[str, Any]:
    """Register a Python callback at a specific parser stage.

    Args:
        stage: One of pre_parse, pre_verb, post_verb, pre_params,
               post_params, pre_content, post_content, post_parse.
        callback: A Python callable that receives a state dict.

    Returns:
        dict with keys: registered, stage, callback_type.
    """
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return json.loads(c.register_stage_guard_py(stage, callback))


# ── K0mmand3r parser ──────────────────────────────────────────────────────────

def parse_kmdline(input_str: str) -> Dict[str, Any]:
    """Parse a k0mmand3r slash command string (legacy API).

    Returns a dict with keys: verb, params, content, rest.
    On parse failure, returns {"error": "<message>"}.

    Examples:
        >>> cmd = parse_kmdline("@b00t(\"whoami\");")
        >>> assert cmd["verb"] == "b00t"
        >>> assert cmd["params"] == {"": "whoami"}
    """
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return c.parse_kmdline(input_str)


def parse_k0mmand3r(input_str: str) -> Dict[str, Any]:
    """Parse a k0mmand3r slash command string (new API).

    Returns serialized KmdLine as dict with keys: verb, params, content.
    """
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return json.loads(c.parse_k0mmand3r(input_str))


# ── Existing MCP/model/datum API (unchanged) ───────────────────────────────────

def mcp_list(path: str = "~/.dotfiles/_b00t_", json_output: bool = False) -> str:
    """List all MCP servers available in the b00t configuration."""
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return c.mcp_list_py(path, json_output)


def mcp_output(servers: str, path: str = "~/.dotfiles/_b00t_", json_format: bool = False) -> str:
    """Get MCP server output in specified format."""
    c = _get_core()
    if c is None:
        raise B00tError("Native b00t_py module not available")
    return c.mcp_output_py(servers, path, json_format)


# ── Fluent interface classes ───────────────────────────────────────────────────

class McpQuery:
    """Fluent interface for MCP operations."""

    def __init__(self, path: str = "~/.dotfiles/_b00t_"):
        self.path = path
        self._servers: Optional[List[str]] = None
        self._json_format = False

    def servers(self, server_list: List[str]) -> "McpQuery":
        """Filter to specific servers."""
        self._servers = server_list
        return self

    def json(self) -> "McpQuery":
        """Use JSON format output."""
        self._json_format = True
        return self

    def list(self) -> str:
        """Execute list operation."""
        return mcp_list(self.path, self._json_format)

    def output(self) -> str:
        """Execute output operation."""
        if self._servers is None:
            raise B00tError("No servers specified. Use .servers() first.")
        server_str = ",".join(self._servers)
        return mcp_output(server_str, self.path, self._json_format)


class AiQuery:
    """Fluent interface for AI operations."""

    def __init__(self, path: str = "~/.dotfiles/_b00t_"):
        self.path = path

    def list(self) -> List[Dict[str, Any]]:
        return []

    def providers(self, provider_list: List[str]) -> "AiQuery":
        return self


class CliQuery:
    """Fluent interface for CLI operations."""

    def __init__(self, path: str = "~/.dotfiles/_b00t_"):
        self.path = path

    def detect(self, tool: str) -> str:
        return f"Tool {tool} detection not yet implemented"


# Factory functions for fluent interface
def mcp(path: str = "~/.dotfiles/_b00t_") -> McpQuery:
    """Create MCP query builder."""
    return McpQuery(path)


def ai(path: str = "~/.dotfiles/_b00t_") -> AiQuery:
    """Create AI query builder."""
    return AiQuery(path)


def cli(path: str = "~/.dotfiles/_b00t_") -> CliQuery:
    """Create CLI query builder."""
    return CliQuery(path)


# ── Re-export exception ────────────────────────────────────────────────────────

__all__ = [
    # Core new API
    "EmojiRegistry",
    "check_guards",
    "parse_kmdline",
    # Existing MCP API
    "mcp_list",
    "mcp_output",
    "mcp",
    "ai",
    "cli",
    "McpQuery",
    "AiQuery",
    "CliQuery",
    "B00tError",
    "__version__",
]
