# b00t bug capture — auto-populated by operator/hooks
# Format: JSONL, one entry per line
# Fields: ts, agent, cmd, error, hint, resolved (optional)
# Query ontology: b00t grok ask "<cmd>" -t <topic>
# Resolve: b00t lfmf datum abstract "<lesson>" && rm .bugs/<date>.jsonl

