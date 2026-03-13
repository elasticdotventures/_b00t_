# Mistral.rs + Candle + Ralph Integration (b00t)

## Proposed mechanism (implemented)

1. Keep b00t's provider abstraction unchanged.
2. Run `mistralrs-server` as a local OpenAI-compatible endpoint.
3. Represent local runtime in b00t datums:
   - CLI datum: `_b00t_/mistralrs.cli.toml`
   - AI provider datum: `_b00t_/mistralrs-local.ai.toml`
   - AI model datum: `_b00t_/mistral-7b-instruct-v0_3-local.ai_model.toml`
4. Use a b00t-native Ralph loop entrypoint at `b00t.sh` target:
   - `ralphs/ralph-plus-_b00t_/ralph.sh`
   - supports `--tool mistralrs` for local inference
   - keeps loop status in `.b00t/ralph/status.json`
   - returns `75` on unfinished max-iteration to align with `b00t-cli up` restart semantics

This is the lowest-risk path because it reuses existing b00t datum/model plumbing and avoids forcing a Rust runtime rework.

## Why this is the right fit

- `b00t-cli up` already expects an outer loop script (`b00t.sh`) and exit code contract (`0` complete, `75` tempfail/restart).
- `ModelProvider::OpenAICompatible` already exists in `b00t-c0re-lib`.
- `mistral.rs` already exposes OpenAI-compatible serving mode; Candle stays under the hood.

## Usage

1. Install local runtime:
   - `b00t-cli cli install mistralrs`
2. Cache model:
   - `b00t-cli model download mistral-7b-instruct-v0_3-local`
3. Start local server:
   - `just mistralrs-up`
4. Smoke test:
   - `just mistralrs-chat prompt="give one line status"`
5. Run Ralph with local inference:
   - `b00t-cli up --tool mistralrs --max-iter 5`

## Future extension (recommended)

- Add `b00t-cli model serve --runtime mistralrs` so vLLM and mistral.rs are first-class runtime choices under one command.
