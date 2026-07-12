---
unsloth-moe-a100-step-time: 30B MoE step time ~130s on A100 regardless of seq_len or lora_r. MoE routing scatter kills GPU cache. Fix: H200 flavor (~50s/step, fits 10h budget).
