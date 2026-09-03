# Bigger-GPU-Justification

The office wants a larger GPU. The honest case, with the cloud PAYG alternative
priced in (verified 2026-09-03).

## What the RTX 3090 (24 GB) can and can't do

| Can | Can't |
|---|---|
| Qwen3.6-27B Q4_K_M, 128K ctx, ~40 tok/s (MTP spec-decode) | run it AND a second model — VRAM is 21 GB used at idle |
| one ch0nky slot; pi/opencode swap for it | serve embeddings + ch0nky concurrently (why `qwen3-embed-local` is inactive) |
| heat the office (~250–300 W under load) | fit a 70B / GLM-class model at usable quant |
| OOM under memory pressure and take the Bash tool with it (happened this session) | headroom for LMCache / vLLM / batch |

A 48 GB card (A6000 / L40S / RTX 6000 Ada / 2×3090-NVLink) buys: ch0nky **+**
embeddings **+** a reranker concurrently; 70B-Q4 or GLM-Air; vLLM with real
batching + prefix cache; no swap-death.

## Cloud PAYG alternative (per 1M tokens)

| Provider / model | In | Out | Notes |
|---|---|---|---|
| **Telnyx** `zai-org/GLM-5.3-Flash` (320B, 1M ctx) | $0.075 | $0.25 | cheapest capable; [[Home\|telnyx-inference datum]] |
| Telnyx `deepseek-ai/DeepSeek-V4-Flash` (284B) | $0.13 | $0.26 | biggest bang/$ |
| Telnyx `Qwen/Qwen3.8-27B` (262K ctx, vision) | $0.40 | $3.00 | cloud twin of the local slot |
| Telnyx `zai-org/GLM-5.3` (753B, 1M ctx) | $1.40 | $4.40 | frontier-tier |
| OpenRouter `qwen/qwen3.8-27b` | $0.25 | $2.20 | alt route |
| OpenRouter `qwen/qwen3-coder` (480B-A35B) | $0.30 | $1.00 | coding |

## The math

- A 48 GB card is ~$4–8k capex + ~$0.10/h power. Amortised over 3 years ≈ **$0.20–0.35/h**.
- At Telnyx GLM-5.3-Flash rates, that hourly cost ≈ **~1–1.5 M output tokens/hour** of cloud spend.
- The hive's actual sustained agent throughput is far below that today (one ch0nky slot, ~40 tok/s ≈ 0.14 M tok/h).
- **So on token economics alone, cloud PAYG wins right now.** The GPU justifies itself on the things cloud can't give: data locality (no prompt egress), latency floor, always-on with no per-token meter, running the [[GPU-Heater-Tasks|heater/embeddings/reranker]] stack concurrently — and, per the operator, **it heats the office**, which a cloud API does not.

## Recommendation

1. Wire the [[Home\|telnyx-inference]] + `openrouter` Qwen3.8/GLM datums as `ch0nky`
   cloud fallback **now** (done for the datums; router wiring is a task).
2. Let the [[GPU-Heater-Tasks|heater]] + poller run — they produce the utilisation
   graph that actually justifies (or doesn't) the capex.
3. Buy the 48 GB card when the poller shows the 3090 pinned >70 % for whole
   working days AND a concrete workload needs concurrency (embeddings + ch0nky +
   rerank) that time-slicing can't cover.

Back to [[Home]].
