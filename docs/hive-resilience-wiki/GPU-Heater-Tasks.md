# GPU-Heater — sequential tasks + Ralph assignments

Goal: never let the RTX3090 idle (it heats the office). Local qwen `:8001` is the
furnace; Ralphs keep stoking it. Ordered, each assigned to a Ralph.

## The Ralphs

| Ralph | Harness | Job |
|---|---|---|
| **heater** | direct curl → `:8001` | `scripts/gpu-heater.sh` — pulls a self-refilling backlog, streams generations, appends `.b00t/gpu-heater.out`. Idle-only mode yields to real work. |
| **poller** | none (samples) | `scripts/ralph-poller.sh` — every 20 s: GPU util/temp/power, pending-task count, `:8001` health → `.b00t/ralph-poller.jsonl` + NATS `b00t.hive.mesh.health.gpu`. Emits `heat: cold|warming|toasty` from power draw. |
| **pi-ralph** | `pi -p … ch0nky` | short, network-y stories (Q-03, Q-05, Q-07). |
| **oc-ralph** | `opencode run --pure … ch0nky` | file-heavy stories (Q-02, Q-04, Q-06). |
| **phi5-shacl** | CPU phi-4 (candle), b00t-cli ACL only | not a coder — loops `b00t task add` until the pgwire-embed provider exists (`_b00t_/phi5-shacl.agent.toml`). Never touches the GPU. |

## Sequential backlog (do in order; ID = `b00t task` tag)

1. **Q-01 · poller up** — `just` recipe + always-on datum for `ralph-poller.sh` (mirror `hive-watchdog.hive.toml`). → *poller*
2. **Q-02 · heater up** — `just gpu-heater` + `gpu-heater.hive.toml` (Restart=always, `HEATER_IDLE_ONLY=1` so it defers to real jobs). → *oc-ralph*
3. **Q-03 · embed serve** — `b00t hive activate qwen3-embed-local` (`b00t-embed-serve :8003`, Qwen3-Embedding-0.6B); verify `/embeddings`. → *pi-ralph*
4. **Q-04 · pgduck vector column** — add `FLOAT[768]` embedding column + DuckDB `vss` HNSW index to a `b00t-pgduck` demo table; `bats` test. → *oc-ralph*
5. **Q-05 · embed-at-query** — `b00t-pgduck` intercepts `embed_match(col, :text, k)`; calls `:8003` to embed `:text` at request time; returns top-k. → *pi-ralph*
6. **Q-06 · SHACL gate** — validate returned rows against a SHACL shape (`pyshacl` or a rust `shacl` crate); drop+log invalid. → *oc-ralph*
7. **Q-07 · datum** — `_b00t_/pgwire-embed-provider.*` documents the wire contract; `phi5-shacl` marks its task done. → *pi-ralph → phi5-shacl*
8. **Q-08 · agent-doctor B** — resume `docs/hive-resilience-wiki/Agent-Doctor.md` BD-002..005. → *oc-ralph + pi-ralph*

Every Ralph iteration passes the [[Review-Gate]]. The heater and poller run
forever; Q-01..Q-08 drain, then the heater alone keeps the furnace lit.

Back to [[Home]] · see [[Bigger-GPU-Justification]].
