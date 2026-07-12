# DARE: Cloud-Coder Training Timeout Strategy

**Context:** Job `6a40eeab81727949c74c49c3` running on A100-large, 10h timeout.
Step 7/534 at 119s/it. 534 total steps × 119s = 17.7h — exceeds 10h timeout.

---

## Decision: Let current run hit timeout, resume from checkpoint-200

---

## Alternatives

### Option A: Run to timeout → resume from checkpoint-200 (RECOMMENDED)
| Metric | Value |
|--------|-------|
| Current progress | step 7/534 |
| Steps to checkpoint-200 | 193 |
| Time to checkpoint-200 | 193 × 119s = **6.4h** |
| Current run total (to ~step 300) | ~10h (timeout) |
| Resume remaining steps | 334 steps × 117s = **10.9h** |
| Total from now | 6.4h + 10.9h = **17.3h** |
| Cost | 17.3h × $2.50 = **$43.25 (4.3🎂)** |
| Checkpoints | 100, 200 saved to HF Hub |

### Option B: Cancel now, restart fresh with `timeout_hours=19.0`
| Metric | Value |
|--------|-------|
| Current sunk | ~14 min, $0.58 |
| Fresh run steps | 534 |
| Time | 534 × 117s = **17.4h** |
| Cost | 17.4h × $2.50 = **$43.50 (4.4🎂)** |
| Checkpoints | single continuous run |

### Option C: Cancel, switch to H200 flavor (50s/step estimated)
| Metric | Value |
|--------|-------|
| Time | 534 × 50s = **7.4h** |
| Cost | 7.4h × $5.00 = **$37.00 (3.7🎂)** |
| Risk | Unverified step time, config may need tuning |

---

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Step time degrades (thermal throttle) | LOW | A100 is HF-managed, consistent cooling |
| Checkpoint push fails (HF Hub auth) | LOW | `HF_TOKEN` secret already verified working (model loaded) |
| HF Jobs preemption mid-run | LOW | A100 rarely preempted; checkpoints at 100-step intervals |
| Resume fails (state mismatch) | LOW | Unsloth `resume_from_checkpoint` is stable for same config |
| H200 step time unknown | MEDIUM | Would need a test run; 50s is estimated not measured |

---

## Execution Plan

1. **Let current job run** — no action needed, it's running
2. **Monitor** checkpoint saves at steps 100 (3.3h from now) and 200 (6.6h from now)
3. **Wait for timeout** — job will ERROR at ~10h / step ~300
4. **Verify checkpoint** exists: `hf ls elasticdotventures/b00t-qwen3-coder-30b --revision checkpoint-200`
5. **Resume with fixed recipe:**
   ```bash
   just ai-finetune::cloud-resume a100-large \
     checkpoint=elasticdotventures/b00t-qwen3-coder-30b/checkpoint-200 \
     config=config-cloud-coder.yaml \
     timeout_hours=13.0
   ```

---

## Recommendation

**Option A** — cost difference is negligible ($0.25), but:
- Step time is still converging (139→127→124→121→119→119), current 119s may be 2-3% high
- Checkpoint-200 provides a verified recovery point
- No wasted work: steps 1-200 are productive, only steps 200-300 are "wasted" on timeout
- Abandoning Option B/C because H200 untested, fresh restart gains nothing

**Net wasted cost from timeout:** ~100 steps (200→300) × 119s = 3.3h × $2.50 = **$8.25 (0.8🎂)** — acceptable as insurance against unknown restart failures.

---
*b00t:map v1*
*summary: DARE for cloud-coder training timeout — Option A: run to timeout, resume from checkpoint-200*
*tags: dare, decision, cloud, training, timeout, cost*
*tier: sm0l*
*cmds: just ai-finetune::cloud-resume a100-large checkpoint=... config=config-cloud-coder.yaml timeout_hours=13.0*
*complexity: 2*
