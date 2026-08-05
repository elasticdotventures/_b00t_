---
ALL container operations on sm3lly MUST carry --memory and --memory-swap equal caps. The node has: ALL container operations on sm3lly MUST carry --memory and --memory-swap equal caps. The node has 31G RAM and an RTX3090 with 24G VRAM. Uncapped containers have caused 3 OOM crashes and 1 fsck-level filesystem recovery in a single day. Default cap is 8g/8g. GPU training gets 16g/16g with 4 CPUs. Never set --memory-swap higher than --memory (disables swap thrashing on cgroup v2). Use  before any GPU workload to gate resources. <!-- salvaged:no_colon -->

---
Escalate podman run without memory caps from warning to BLOCKED guard after 3 OOM crashes and 1: Escalate podman run without memory caps from warning to BLOCKED guard after 3 OOM crashes and 1 fsck recovery. Every container operation MUST carry --memory and --memory-swap set equal. Default cap 8g/8g, GPU training 16g/16g, maximum 24g/24g. Documented in _b00t_/podman-resource-guard.tomllm <!-- salvaged:no_colon -->
