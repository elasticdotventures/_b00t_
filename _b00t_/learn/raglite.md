---
b00t integration: B00T_DIR env must be set for topic discovery. llama-cpp-python>=0.3.0 required. RAGLiteConfig(db_url=duckdb:///path, llm=openai/ch0nky, embedder=llama-cpp/bge-m3). Invoke: uv pip install raglite.

python import gates: Declare simple Python import requirements as datum-agnostic `[b00t.py.imports] modules = ["a", "b"]`. Use nested `[[b00t.py.imports.packages]]` only when package differs from module or a condition applies. Domain blocks such as `[b00t.raglite]` SHOULD hold configuration facts, not generic Python dependency semantics.

---
venv import probe: Validate grok raglite with ~/.venv/bin/python -c 'import raglite' before debugging query code.

---
b00t venv path: Keep HOME/.venv aligned with B00T_DIR/.venv or grok raglite subprocess spawning fails before import diagnostics.
