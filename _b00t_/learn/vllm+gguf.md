---
qwen3next GGUF needs 5 patches: (1)uv upgrade gguf==0.18.0, (2)transformers/ggml.py add qwen3_next mappings, (3)modeling_gguf_pytorch_utils.py arch remap+tensor processor, (4)gguf_loader.py qwen3_next replace+ssm_dt.bias, (5)qwen3_next.py ParallelLMHead quant_config missing — all in .venv, re-apply after venv rebuild
