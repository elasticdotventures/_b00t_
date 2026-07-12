---
unsloth-moe-step-time-root-cause: 30B step time = f(seq_len, model_size) NOT f(lora_r). Fix: packing=True + max_seq_length=1024 cuts attention O(n²)+FFN ~3-4x.
