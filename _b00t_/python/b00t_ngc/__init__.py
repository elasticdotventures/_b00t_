"""b00t_ngc — NGC + NVIDIA API client for b00t hive tooling."""
from .client import NvidiaClient, ContainerTag, Model, ChatMessage
from ._auth import load_key

__all__ = ["NvidiaClient", "ContainerTag", "Model", "ChatMessage", "load_key"]
