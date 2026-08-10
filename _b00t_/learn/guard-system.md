---
state persistence: Guard violation counters should use append-mode JSONL (OpenOptions::append) instead of rewriting whole file. Each violation appends one line: {\"pattern\": \"$name\", \"count\": $n}. Compaction merges duplicates via just guard-compact. Weekly cron job prevents unbounded growth. Use best-effort IO — never let persistence failure block command execution.

---
canonical types: ufo-types::satisfies (Satisfies<C>, SatisfiesResult, Disposition, EvidenceBridge) — constraint evaluation outcomes now belong to the crate, not hand-rolled shapes here.
