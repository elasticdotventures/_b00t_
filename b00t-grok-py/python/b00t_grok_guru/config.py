"""Configuration for RAG-Anything models and b00t-grok system."""

import os
from typing import Optional, Callable, List, Dict, Any
import logging


class GrokConfig:
    """Configuration for b00t-grok-guru system."""

    def __init__(self):
        # Qdrant configuration
        self.qdrant_url = os.getenv("QDRANT_URL", "http://localhost:6333")
        self.qdrant_api_key = os.getenv("QDRANT_API_KEY", "")

        # RAG-Anything configuration
        self.use_rag_anything = os.getenv("USE_RAG_ANYTHING", "true").lower() == "true"
        self.rag_working_dir = os.getenv("RAG_WORKING_DIR", "/tmp/b00t_grok_rag")

        # Model provider selection (openai, ollama, anthropic, etc.)
        self.llm_provider = os.getenv("LLM_PROVIDER", "openai")
        self.vision_provider = os.getenv("VISION_PROVIDER", "openai")
        self.embedding_provider = os.getenv("EMBEDDING_PROVIDER", "openai")

        # OpenAI configuration
        self.openai_api_key = os.getenv("OPENAI_API_KEY", "")
        self.openai_model = os.getenv("OPENAI_MODEL", "gpt-4o-mini")
        self.openai_vision_model = os.getenv("OPENAI_VISION_MODEL", "gpt-4o-mini")
        self.openai_embedding_model = os.getenv("OPENAI_EMBEDDING_MODEL", "text-embedding-3-small")

        # Ollama configuration (local models)
        self.ollama_url = os.getenv("OLLAMA_URL", "http://localhost:11434")
        self.ollama_model = os.getenv("OLLAMA_MODEL", "llama3.2")
        self.ollama_vision_model = os.getenv("OLLAMA_VISION_MODEL", "llava")
        self.ollama_embedding_model = os.getenv("OLLAMA_EMBEDDING_MODEL", "nomic-embed-text")

        # Anthropic configuration
        self.anthropic_api_key = os.getenv("ANTHROPIC_API_KEY", "")
        self.anthropic_model = os.getenv("ANTHROPIC_MODEL", "claude-3-5-sonnet-20241022")

    def get_llm_func(self) -> Callable:
        """Get LLM function based on provider."""
        if self.llm_provider == "openai":
            return self._create_openai_llm_func()
        elif self.llm_provider == "ollama":
            return self._create_ollama_llm_func()
        elif self.llm_provider == "anthropic":
            return self._create_anthropic_llm_func()
        else:
            logging.warning(f"Unknown LLM provider: {self.llm_provider}, using default")
            return self._create_default_llm_func()

    def get_vision_func(self) -> Callable:
        """Get vision model function based on provider."""
        if self.vision_provider == "openai":
            return self._create_openai_vision_func()
        elif self.vision_provider == "ollama":
            return self._create_ollama_vision_func()
        else:
            logging.warning(f"Unknown vision provider: {self.vision_provider}, using default")
            return self._create_default_vision_func()

    def get_embedding_func(self) -> Callable:
        """Get embedding function based on provider."""
        if self.embedding_provider == "openai":
            return self._create_openai_embedding_func()
        elif self.embedding_provider == "ollama":
            return self._create_ollama_embedding_func()
        else:
            logging.warning(f"Unknown embedding provider: {self.embedding_provider}, using default")
            return self._create_default_embedding_func()

    # OpenAI implementations
    def _create_openai_llm_func(self) -> Callable:
        """Create OpenAI LLM function."""
        try:
            from openai import OpenAI
            client = OpenAI(api_key=self.openai_api_key)

            def llm_func(
                prompt: str,
                system_prompt: Optional[str] = None,
                history_messages: Optional[List[Dict]] = None
            ) -> str:
                messages = []
                if system_prompt:
                    messages.append({"role": "system", "content": system_prompt})
                if history_messages:
                    messages.extend(history_messages)
                messages.append({"role": "user", "content": prompt})

                response = client.chat.completions.create(
                    model=self.openai_model,
                    messages=messages
                )
                return response.choices[0].message.content

            return llm_func
        except ImportError:
            logging.error("OpenAI library not installed")
            return self._create_default_llm_func()

    def _create_openai_vision_func(self) -> Callable:
        """Create OpenAI vision function."""
        try:
            from openai import OpenAI
            import base64

            client = OpenAI(api_key=self.openai_api_key)

            def vision_func(image_path: str, prompt: str) -> str:
                # Read and encode image
                with open(image_path, "rb") as f:
                    image_data = base64.b64encode(f.read()).decode("utf-8")

                response = client.chat.completions.create(
                    model=self.openai_vision_model,
                    messages=[
                        {
                            "role": "user",
                            "content": [
                                {"type": "text", "text": prompt},
                                {
                                    "type": "image_url",
                                    "image_url": {
                                        "url": f"data:image/jpeg;base64,{image_data}"
                                    }
                                }
                            ]
                        }
                    ]
                )
                return response.choices[0].message.content

            return vision_func
        except ImportError:
            logging.error("OpenAI library not installed")
            return self._create_default_vision_func()

    def _create_openai_embedding_func(self) -> Callable:
        """Create OpenAI embedding function."""
        try:
            from openai import OpenAI
            client = OpenAI(api_key=self.openai_api_key)

            def embedding_func(texts: List[str]) -> List[List[float]]:
                response = client.embeddings.create(
                    model=self.openai_embedding_model,
                    input=texts
                )
                return [item.embedding for item in response.data]

            return embedding_func
        except ImportError:
            logging.error("OpenAI library not installed")
            return self._create_default_embedding_func()

    # Ollama implementations
    def _create_ollama_llm_func(self) -> Callable:
        """Create Ollama LLM function."""
        try:
            import requests

            def llm_func(
                prompt: str,
                system_prompt: Optional[str] = None,
                history_messages: Optional[List[Dict]] = None
            ) -> str:
                messages = []
                if system_prompt:
                    messages.append({"role": "system", "content": system_prompt})
                if history_messages:
                    messages.extend(history_messages)
                messages.append({"role": "user", "content": prompt})

                response = requests.post(
                    f"{self.ollama_url}/api/chat",
                    json={
                        "model": self.ollama_model,
                        "messages": messages,
                        "stream": False
                    }
                )
                return response.json()["message"]["content"]

            return llm_func
        except Exception as e:
            logging.error(f"Ollama setup failed: {e}")
            return self._create_default_llm_func()

    def _create_ollama_vision_func(self) -> Callable:
        """Create Ollama vision function."""
        try:
            import requests
            import base64

            def vision_func(image_path: str, prompt: str) -> str:
                with open(image_path, "rb") as f:
                    image_data = base64.b64encode(f.read()).decode("utf-8")

                response = requests.post(
                    f"{self.ollama_url}/api/generate",
                    json={
                        "model": self.ollama_vision_model,
                        "prompt": prompt,
                        "images": [image_data],
                        "stream": False
                    }
                )
                return response.json()["response"]

            return vision_func
        except Exception as e:
            logging.error(f"Ollama vision setup failed: {e}")
            return self._create_default_vision_func()

    def _create_ollama_embedding_func(self) -> Callable:
        """Create Ollama embedding function."""
        try:
            import requests

            def embedding_func(texts: List[str]) -> List[List[float]]:
                embeddings = []
                for text in texts:
                    response = requests.post(
                        f"{self.ollama_url}/api/embeddings",
                        json={
                            "model": self.ollama_embedding_model,
                            "prompt": text
                        }
                    )
                    embeddings.append(response.json()["embedding"])
                return embeddings

            return embedding_func
        except Exception as e:
            logging.error(f"Ollama embedding setup failed: {e}")
            return self._create_default_embedding_func()

    # Anthropic implementation
    def _create_anthropic_llm_func(self) -> Callable:
        """Create Anthropic LLM function."""
        try:
            from anthropic import Anthropic
            client = Anthropic(api_key=self.anthropic_api_key)

            def llm_func(
                prompt: str,
                system_prompt: Optional[str] = None,
                history_messages: Optional[List[Dict]] = None
            ) -> str:
                messages = history_messages or []
                messages.append({"role": "user", "content": prompt})

                response = client.messages.create(
                    model=self.anthropic_model,
                    max_tokens=4096,
                    system=system_prompt or "",
                    messages=messages
                )
                return response.content[0].text

            return llm_func
        except ImportError:
            logging.error("Anthropic library not installed")
            return self._create_default_llm_func()

    # Default/fallback implementations
    def _create_default_llm_func(self) -> Callable:
        """Default LLM function (mock)."""
        def llm_func(prompt: str, **kwargs) -> str:
            logging.warning("Using mock LLM - configure a real provider")
            return f"Mock response to: {prompt[:100]}..."
        return llm_func

    def _create_default_vision_func(self) -> Callable:
        """Default vision function (mock)."""
        def vision_func(image_path: str, prompt: str) -> str:
            logging.warning("Using mock vision - configure a real provider")
            return f"Mock vision analysis of {image_path}"
        return vision_func

    def _create_default_embedding_func(self) -> Callable:
        """Default embedding function (hash-based)."""
        def embedding_func(texts: List[str]) -> List[List[float]]:
            logging.warning("Using mock embeddings - configure a real provider")
            import hashlib
            embeddings = []
            for text in texts:
                text_hash = int(hashlib.md5(text.encode()).hexdigest(), 16)
                base = (text_hash % 10000) / 10000.0
                embeddings.append([base + i * 0.001 for i in range(768)])
            return embeddings
        return embedding_func


# Global config instance
config = GrokConfig()
