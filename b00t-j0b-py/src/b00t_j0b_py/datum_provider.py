"""Provider configuration via b00t datums (DRY - uses Rust via PyO3).

This module provides a DRY interface to AI model providers by leveraging
the b00t datum system via PyO3 bindings. Instead of duplicating provider
logic in Python, we use the native Rust implementation.

Environment variables are validated via datums, not hardcoded checks.
"""

from typing import Dict, List, Optional, Any
import os


# Try to import b00t_py (Rust bindings via PyO3)
try:
    import b00t_py
    HAS_B00T_PY = True
except ImportError:
    HAS_B00T_PY = False
    # Fallback warning
    import warnings
    warnings.warn(
        "b00t_py Rust bindings not available. "
        "Install with: pip install b00t-py OR cd ../b00t-py && maturin develop"
    )


class DatumProvider:
    """Provider configured via b00t datum system (Rust-backed).

    This is the DRY approach - no duplicate provider logic.
    All provider configuration lives in TOML datums in ~/.dotfiles/_b00t_/
    and is accessed via Rust (via PyO3).
    """

    def __init__(self, model_name: str, datum_path: str = "~/.dotfiles/_b00t_"):
        """Initialize provider from model datum.

        Args:
            model_name: Name of model (e.g., "qwen-2.5-72b", "claude-3-5-sonnet")
            datum_path: Path to b00t datum directory

        Raises:
            ImportError: If b00t_py not available
            ValueError: If model datum not found or invalid
        """
        if not HAS_B00T_PY:
            raise ImportError(
                "b00t_py required for datum providers. "
                "Install: pip install b00t-py"
            )

        self.model_name = model_name
        self.datum_path = datum_path

        # Load model datum via Rust
        try:
            self.datum = b00t_py.load_ai_model_datum(model_name, datum_path)
        except Exception as e:
            raise ValueError(f"Failed to load model datum '{model_name}': {e}")

        # Extract key fields
        self.provider = self.datum.get("provider", "unknown")
        self.api_base = self.datum.get("api_base")
        self.api_key_env = self.datum.get("api_key_env")
        self.litellm_model = self.datum.get("litellm_model", "")

    def validate_env(self) -> tuple[bool, List[str]]:
        """Validate required environment variables via Rust datum system.

        Returns:
            (is_valid, missing_vars): Tuple of validation status and missing vars
        """
        if not HAS_B00T_PY:
            return False, ["b00t_py not available"]

        try:
            # Check provider env vars via Rust
            result = b00t_py.check_provider_env(self.provider, self.datum_path)
            return result["available"], result.get("missing_env_vars", [])
        except Exception:
            # Fallback: check api_key_env directly
            if self.api_key_env:
                is_set = os.getenv(self.api_key_env) is not None
                missing = [] if is_set else [self.api_key_env]
                return is_set, missing
            return True, []  # No env vars required

    def get_api_key(self) -> Optional[str]:
        """Get API key from environment (validated by datum).

        Returns:
            API key or None if not set
        """
        if self.api_key_env:
            return os.getenv(self.api_key_env)
        return None

    def to_model_config(self) -> Dict[str, Any]:
        """Convert datum to model configuration dict.

        Returns:
            Model configuration for ADK/LiteLLM
        """
        return {
            "model_name": self.litellm_model,
            "provider": self.provider,
            "api_base": self.api_base,
            "api_key": self.get_api_key(),
            "parameters": self.datum.get("parameters", {}),
            "capabilities": self.datum.get("capabilities", []),
            "context_window": self.datum.get("context_window", 4096),
        }

    @staticmethod
    def list_available_providers() -> List[str]:
        """List all available AI providers from datums.

        Returns:
            List of provider names (e.g., ["openrouter", "anthropic", "ollama"])
        """
        if not HAS_B00T_PY:
            return []

        try:
            return b00t_py.list_ai_providers()
        except Exception:
            return []

    @staticmethod
    def list_available_models() -> List[str]:
        """List all available AI models from datums.

        Returns:
            List of model names (e.g., ["qwen-2.5-72b", "claude-3-5-sonnet"])
        """
        if not HAS_B00T_PY:
            return []

        try:
            return b00t_py.list_ai_models()
        except Exception:
            return []

    @staticmethod
    def select_model_by_capability(
        capability: str,
        prefer_local: bool = False,
        max_cost: Optional[float] = None,
    ) -> Optional[str]:
        """Select best model matching criteria (agent self-selection).

        This enables agents to select models based on task requirements,
        cost constraints, or availability.

        Args:
            capability: Required capability (e.g., "reasoning", "code", "long_context")
            prefer_local: Prefer local models (Ollama, etc.)
            max_cost: Maximum cost per 1K tokens

        Returns:
            Model name or None if no match
        """
        if not HAS_B00T_PY:
            return None

        models = DatumProvider.list_available_models()

        # Filter by capability and constraints
        candidates = []
        for model_name in models:
            try:
                provider = DatumProvider(model_name)
                config = provider.to_model_config()

                # Check capability
                if capability not in config.get("capabilities", []):
                    continue

                # Check env vars (must be available)
                is_valid, _ = provider.validate_env()
                if not is_valid:
                    continue

                # Check local preference
                if prefer_local and config["provider"] != "ollama":
                    continue

                # Check cost (would need cost metadata in datum)
                # TODO: Add cost filtering when datum has cost info

                candidates.append((model_name, config))
            except Exception:
                continue

        if not candidates:
            return None

        # Sort by preference (local first if preferred, then by cost)
        if prefer_local:
            candidates.sort(key=lambda x: (
                0 if x[1]["provider"] == "ollama" else 1,
                x[1].get("cost_per_1k", 999),
            ))
        else:
            candidates.sort(key=lambda x: x[1].get("cost_per_1k", 999))

        return candidates[0][0]


def create_agent_from_datum(
    model_name: str,
    task: str,
    tools: Optional[List[str]] = None,
    **kwargs
) -> Dict[str, Any]:
    """Create ADK agent configuration from datum (DRY helper).

    Args:
        model_name: Model datum name (e.g., "qwen-2.5-72b")
        task: Task description
        tools: Tool names to enable
        **kwargs: Additional agent config

    Returns:
        Agent configuration dict for adk_agent_job

    Example:
        >>> config = create_agent_from_datum(
        ...     "qwen-2.5-72b",
        ...     "Research quantum computing",
        ...     tools=["search", "calculator"]
        ... )
        >>> job = queue.enqueue(adk_agent_job, agent_config_dict=config, task=task)
    """
    provider = DatumProvider(model_name)

    # Validate env vars
    is_valid, missing = provider.validate_env()
    if not is_valid:
        raise EnvironmentError(
            f"Provider '{provider.provider}' missing required env vars: {missing}. "
            "Set these in .envrc or .env"
        )

    # Build config from datum
    model_config = provider.to_model_config()

    agent_config = {
        "name": f"{model_name}-agent",
        "description": kwargs.get("description", f"Agent powered by {model_name}"),

        # From datum
        "provider": provider.provider,
        "model_name": model_config["model_name"],
        "api_base": model_config.get("api_base"),
        "api_key": None,  # Will use env var

        # Agent settings
        "tools": tools or [],
        "temperature": kwargs.get("temperature", model_config["parameters"].get("temperature", 0.7)),
        "max_tokens": kwargs.get("max_tokens", model_config["parameters"].get("max_tokens", 4096)),

        # Optional
        **{k: v for k, v in kwargs.items() if k not in ("description", "temperature", "max_tokens")}
    }

    return agent_config
