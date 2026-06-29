---
MLflow not installed in docker.io/unsloth/unsloth: latest (ships vLLM variant). Setting report_to:mlflow without mlflow package causes RuntimeError at SFTTrainer.__init__ before any training step. Fix: bake mlflow into custom training image via uv pip install, or set report_to:none.
