#!/usr/bin/env python3
"""
Semantic search across b00t datums using embed_anything

Usage:
    python semantic_datum_search.py --query "how to install kubernetes"
    python semantic_datum_search.py --embed-all --output datums.json
"""

import argparse
import json
import sys
from pathlib import Path

# 🤓: tomllib only in Python 3.11+, fallback to tomli for 3.10
if sys.version_info >= (3, 11):
    import tomllib
else:
    try:
        import tomli as tomllib
    except ImportError:
        print("⚠️  tomli not installed. Run: uv pip install tomli")
        sys.exit(1)
from typing import List, Dict, Any
from dataclasses import dataclass, asdict

try:
    from embed_anything import EmbeddingModel, WhichModel, TextEmbedConfig
    EMBED_ANYTHING_AVAILABLE = True
except ImportError:
    EMBED_ANYTHING_AVAILABLE = False
    print("⚠️  embed_anything not installed. Run: uv pip install embed-anything")


@dataclass
class DatumEmbedding:
    """Embedded datum with metadata"""
    name: str
    type: str
    hint: str
    embedding: List[float]
    learn_content: str = ""
    file_path: str = ""


class DatumEmbedder:
    """Embed b00t datums for semantic search"""

    def __init__(self, b00t_path: str = "~/.b00t/_b00t_",
                 model_id: str = "sentence-transformers/all-MiniLM-L6-v2"):
        self.b00t_path = Path(b00t_path).expanduser()
        self.model_id = model_id
        self.model = None

        if EMBED_ANYTHING_AVAILABLE:
            print(f"🧠 Loading embedding model: {model_id}")
            self.model = EmbeddingModel.from_pretrained_hf(
                WhichModel.Bert,
                model_id=model_id
            )

    def load_datums(self) -> List[Dict[str, Any]]:
        """Load all datum TOML files"""
        datums = []

        for toml_file in self.b00t_path.glob("*.toml"):
            try:
                with open(toml_file, "rb") as f:
                    datum = tomllib.load(f)

                if "b00t" in datum:
                    b00t_data = datum["b00t"]
                    datums.append({
                        "name": b00t_data.get("name", toml_file.stem),
                        "type": b00t_data.get("type", "unknown"),
                        "hint": b00t_data.get("hint", ""),
                        "learn": b00t_data.get("learn", {}),
                        "file_path": str(toml_file)
                    })
            except Exception as e:
                print(f"⚠️  Failed to load {toml_file.name}: {e}")

        print(f"✅ Loaded {len(datums)} datums")
        return datums

    def get_embeddable_text(self, datum: Dict[str, Any]) -> str:
        """Extract text for embedding from datum"""
        parts = []

        # Core metadata
        parts.append(f"Name: {datum['name']}")
        parts.append(f"Type: {datum['type']}")
        parts.append(f"Description: {datum['hint']}")

        # Learn content if available
        learn = datum.get("learn", {})
        if isinstance(learn, dict):
            if "topic" in learn:
                parts.append(f"Topic: {learn['topic']}")
            if "inline" in learn:
                parts.append(f"Documentation: {learn['inline']}")

        return "\n".join(parts)

    def embed_datums(self, datums: List[Dict[str, Any]]) -> List[DatumEmbedding]:
        """Generate embeddings for all datums"""
        if not self.model:
            raise RuntimeError("embed_anything not available")

        embedded_datums = []

        for datum in datums:
            text = self.get_embeddable_text(datum)

            # Embed single query (datum as query)
            embedding = self.model.embed_query([text], config=None)

            embedded_datums.append(DatumEmbedding(
                name=datum["name"],
                type=datum["type"],
                hint=datum["hint"],
                embedding=embedding[0].embedding,  # First result
                learn_content=text,
                file_path=datum["file_path"]
            ))

        print(f"✅ Generated {len(embedded_datums)} embeddings")
        return embedded_datums

    def semantic_search(self, query: str, embedded_datums: List[DatumEmbedding],
                       top_k: int = 5) -> List[tuple[DatumEmbedding, float]]:
        """Search datums by semantic similarity"""
        if not self.model:
            raise RuntimeError("embed_anything not available")

        # Embed query
        query_embedding = self.model.embed_query([query], config=None)[0].embedding

        # Compute cosine similarity
        import numpy as np

        results = []
        for datum in embedded_datums:
            # Cosine similarity
            similarity = np.dot(query_embedding, datum.embedding) / (
                np.linalg.norm(query_embedding) * np.linalg.norm(datum.embedding)
            )
            results.append((datum, float(similarity)))

        # Sort by similarity descending
        results.sort(key=lambda x: x[1], reverse=True)

        return results[:top_k]


def main():
    parser = argparse.ArgumentParser(description="Semantic search for b00t datums")
    parser.add_argument("--query", type=str, help="Search query")
    parser.add_argument("--embed-all", action="store_true",
                       help="Embed all datums and save")
    parser.add_argument("--output", type=str, default="datum_embeddings.json",
                       help="Output file for embeddings")
    parser.add_argument("--input", type=str,
                       help="Load pre-computed embeddings from file")
    parser.add_argument("--top-k", type=int, default=5,
                       help="Number of results to return")
    parser.add_argument("--model", type=str,
                       default="sentence-transformers/all-MiniLM-L6-v2",
                       help="Embedding model ID")

    args = parser.parse_args()

    if not EMBED_ANYTHING_AVAILABLE:
        print("❌ embed_anything not installed")
        print("Install with: uv pip install embed-anything")
        return 1

    embedder = DatumEmbedder(model_id=args.model)

    # Embed all datums
    if args.embed_all:
        datums = embedder.load_datums()
        embedded = embedder.embed_datums(datums)

        # Save to JSON
        output_data = [asdict(d) for d in embedded]
        with open(args.output, "w") as f:
            json.dump(output_data, f, indent=2)

        print(f"✅ Saved {len(embedded)} embeddings to {args.output}")
        return 0

    # Semantic search
    if args.query:
        # Load embeddings
        if args.input and Path(args.input).exists():
            print(f"📂 Loading embeddings from {args.input}")
            with open(args.input) as f:
                data = json.load(f)
                embedded = [DatumEmbedding(**d) for d in data]
        else:
            print("🔄 Generating embeddings...")
            datums = embedder.load_datums()
            embedded = embedder.embed_datums(datums)

        # Search
        results = embedder.semantic_search(args.query, embedded, top_k=args.top_k)

        print(f"\n🔍 Search results for: '{args.query}'\n")
        for idx, (datum, score) in enumerate(results, 1):
            print(f"{idx}. {datum.name} ({datum.type}) - similarity: {score:.3f}")
            print(f"   {datum.hint}")
            print(f"   📁 {datum.file_path}\n")

        return 0

    parser.print_help()
    return 1


if __name__ == "__main__":
    exit(main())
