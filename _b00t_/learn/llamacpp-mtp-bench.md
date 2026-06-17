---
Qwen3.6-27B MTP (Q4_K_M, dnm2) on 3090: TPS 46→25→3.5→1.7 as ctx 0→2K→8K→16K. TTFT 227ms→67s. MTP degrades >8K ctx. Gemma4 no-MTP: 22→3.2→2.9→0. vLLM FP8 OOM on 3090. Only INT4 fits single GPU for vLLM MTP.

Benchmark notes: MTP (Multi-Token Prediction) viable only <8K ctx on RTX3090. Beyond 8K, speculative decode overhead exceeds savings. vLLM FP8 OOM; INT4 GGUF only option for single-GPU hive nodes.

# b00t:map v1
# summary: llamacpp MTP benchmark — Qwen3.6-27B Q4_K_M on RTX3090, TPS vs ctx, TTFT
# tags: benchmark, llamacpp, mtp, qwen, vllm, tps, rtx3090, inference
# tier: sm0l
# cmds: b00t learn llamacpp-mtp-bench
# complexity: 2
