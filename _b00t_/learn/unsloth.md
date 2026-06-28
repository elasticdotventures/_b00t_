---
YAML `2e-4` loads as string in older PyYAML; always cast config numeric values: float(config.get("learning_rate", 2e-4)), int(config.get("num_epochs", 3)). The unsloth container's bitsandbytes raises TypeError on string lr.

---
OOM training 0.5B alongside ch0nky: use batch=1 grad_accum=16 max_seq=512 (not batch=4 max_seq=2048)
