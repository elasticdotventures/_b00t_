---
memoize: "Solve Once, Memoize Forever. The first time you solve a problem, it costs LLM tokens. Every subsequent time MUST cost zero tokens — the solution is cached in a just recipe, a datum, a fine-tuned weight, or a CarmackSolution. If you solve the same problem twice with an LLM, you failed."

---
no-call: "The Fastest LLM Call Is No Call. Before invoking the model, check: is this answer memoized? Can a just recipe handle it? Is there a datum? A fine-tuned reflex? Only invoke the LLM when the answer is genuinely novel."

---
energy: "Measure Energy, Not Tokens. Tokens are a proxy. Energy is the real cost — GPU watts, context window pollution, attention decay. Track EnergyBudget (gpu_watt_seconds, llm_invocations, memoization_hits, efficiency_ratio). A 5-token just call costs 0 GPU watts. A 500-token LLM call costs ~0.3 GPU-seconds."

---
determinism: "Determinism Is a Feature, Not a Constraint. Non-deterministic LLM output is a bug. Every output should be reproducible. If the same prompt produces different results, the prompt is underspecified. Add constraints until output is deterministic."

---
fine-tune-cache: "The Fine-Tune Is the Ultimate Cache. A just recipe memoizes a command. A datum memoizes knowledge. A fine-tuned weight memoizes BEHAVIOR — the model itself becomes the cache. Goal: train model to emit `just submodule-status` (5 tokens) instead of the full bash pipeline (50 tokens). The TrainingCorpus is the ultimate Carmack cache."

---
entangle: "CarmackSolution entangles Solutions with EnergyBudget. Track recall_count (how many times the solution was recalled without re-solving), energy_saved (GPU watt-seconds avoided), and effective_cost(). A solution with recall_count=100 and zero LLM invocations is a perfect Carmack solution."

---
leave-for-next: "Set the Next Agent Up for Success. Leave a recipe (just submodule-status), not an explanation. Leave a datum (_b00t_/learn/john-carmack.md), not a conversation. Leave a test (cargo test -p ufo-types), not a guess. Leave a fine-tune example (corpus/), not a re-explanation. The best Carmack agent optimizes for the NEXT agent's context window."

# b00t:map v1
# summary: Carmack's laws of LLM efficiency — memoize, measure energy, be deterministic, cache in fine-tuned weights, leave recipes for the next agent
# tags: carmack, memoization, energy, determinism, fine-tune, optimization, agent
# tier: frontier
# cmds: just ufo-test, just submodule-status, b00t learn john-carmack
# complexity: 7
