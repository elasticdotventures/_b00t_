# ── HF Cloud — dataset sync + job lifecycle ────────────────────────────────────
# Extracted from root justfile for module isolation.

# Upload train.jsonl + ai-finetune.just + active config to HF dataset repo (source of truth for cloud jobs)
hf-dataset-push config="":
    #!/usr/bin/env bash
    set -euo pipefail
    hf upload elasticdotventures/b00t-training fine-tune/train.jsonl train.jsonl --repo-type dataset
    hf upload elasticdotventures/b00t-training fine-tune/train_unsloth.py train_unsloth.py --repo-type dataset
    hf upload elasticdotventures/b00t-training _b00t_/ai-finetune.just ai-finetune.just --repo-type dataset
    if [[ -n "{{config}}" ]]; then
        hf upload elasticdotventures/b00t-training "fine-tune/{{config}}" "{{config}}" --repo-type dataset
        echo "✅ pushed train.jsonl + train_unsloth.py + ai-finetune.just + {{config}}"
    else
        echo "✅ pushed train.jsonl + train_unsloth.py + ai-finetune.just (no config — pass config=<file> to include)"
    fi

# Submit a training job to HF Jobs; pass config=<filename> and flavor=a10g-large|a100-large
# 🤓 job loads ai-finetune.just + config from hf://datasets/elasticdotventures/b00t-training
hf-job-submit config flavor="a10g-large":
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -z "{{config}}" ]] && { echo "⚠️  pass config=<filename>  e.g. just hf-job-submit config=config-phi4-mini.yaml" >&2; exit 1; }
    hf jobs run \
      --image ghcr.io/elasticdotventures/b00t-training-image:latest \
      --flavor "{{flavor}}" \
      --timeout 10h \
      --env HF_HOME=/tmp/hf-cache \
      --env UNSLOTH_COMPILE_LOCATION=/tmp/unsloth_compiled_cache \
      --env UNSLOTH_VLLM_STANDBY=0 \
      --secret HF_TOKEN \
      --volume "hf://datasets/elasticdotventures/b00t-training:/data:ro" \
      -- /bin/sh -c "command -v just 2>/dev/null || { curl -sSL -o /tmp/just.tar.gz https://github.com/casey/just/releases/download/1.54.0/just-1.54.0-x86_64-unknown-linux-musl.tar.gz && tar -xzf /tmp/just.tar.gz -C /tmp just; }; \$(command -v just 2>/dev/null || echo /tmp/just) -f /data/ai-finetune.just train /data/{{config}}"

# Poll status of a HF job
hf-job-status job_id:
    hf jobs status {{job_id}}

# Stream logs from a running or completed HF job
hf-job-logs job_id:
    hf jobs logs {{job_id}}

# Cancel a running HF job
hf-job-cancel job_id:
    hf jobs cancel {{job_id}}

# Pull completed adapter from HF Hub to local sm3lly
hf-adapter-pull repo dest="fine-tune/output-cloud/lora-adapter":
    hf download "{{repo}}" --repo-type model --local-dir "{{dest}}"

# Hugging Face model caching helper
hf-download model dest="" revision="":
	#!/usr/bin/env bash
	set -euo pipefail
	MODEL="{{model}}"
	if [[ -z "$MODEL" ]]; then
		echo "⚠️ set model=<repo>" >&2
		exit 1
	fi
	# 🤓 prefer hf (huggingface_hub>=0.26 alias); auto-install if missing
	if ! command -v hf >/dev/null 2>&1; then
		echo "hf not found — auto-installing huggingface_hub[cli] via uv ..." >&2
		uv tool install --upgrade "huggingface_hub[cli]"
	fi
	DEST="{{dest}}"
	if [[ -z "$DEST" ]]; then
		SANITIZED="${MODEL//\//__}"
		DEST="$HOME/.b00t/models/$SANITIZED"
	fi
	mkdir -p "$DEST"
	ARGS=(download "$MODEL" --local-dir "$DEST" --local-dir-use-symlinks False)
	if [[ -n "{{revision}}" ]]; then
		ARGS+=(--revision "{{revision}}")
	fi
	hf "${ARGS[@]}"
	echo "✅ cached $MODEL -> $DEST"

# Invoke b00t-cli to install/cache a datum-backed model
b00t-install-model model="llava" force="false" no_activate="false":
	#!/usr/bin/env bash
	set -euo pipefail
	MODEL="{{model}}"
	ARGS=(model download "$MODEL")
	if [[ "{{force}}" == "true" ]]; then
		ARGS+=(--force)
	fi
	if [[ "{{no_activate}}" == "true" ]]; then
		ARGS+=(--no-activate)
	fi
	b00t-cli "${ARGS[@]}"

# Launch vLLM container against cached weights
vllm-up model="" dtype="" port="8000" image="vllm/vllm-openai:latest":
	#!/usr/bin/env bash
	set -euo pipefail
	if [[ -n "{{model}}" ]]; then
		eval "$(b00t-cli model env "{{model}}")"
	else
		eval "$(b00t-cli model env)"
	fi
	: "${VLLM_MODEL_DIR:?Missing VLLM_MODEL_DIR from model env}"
	: "${VLLM_MODEL_PATH:?Missing VLLM_MODEL_PATH from model env}"
	DTYPE="${dtype:-${VLLM_DTYPE:-float16}}"
	PORT="{{port}}"
	IMAGE="{{image}}"
	CONTAINER="${VLLM_CONTAINER_NAME:-vllm-server}"
	docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
	EXTRA_ARGS=()
	if [[ -n "${VLLM_MAX_MODEL_LEN:-}" ]]; then
		EXTRA_ARGS+=(--max-model-len "${VLLM_MAX_MODEL_LEN}")
	fi
	if [[ -n "${VLLM_EXTRA_ARGS:-}" ]]; then
		# shellcheck disable=SC2206
		EXTRA_ARGS+=(${VLLM_EXTRA_ARGS})
	fi
	podman run --rm -d \
		--name "$CONTAINER" \
		--device nvidia.com/gpu=all \
		-p "${PORT}:8000" \
		-v "${VLLM_MODEL_DIR}:${VLLM_MODEL_PATH}:ro" \
		${HF_TOKEN:+-e HF_TOKEN="$HF_TOKEN"} \
		"$IMAGE" \
		--model "${VLLM_MODEL_PATH}" \
		--dtype "${DTYPE}" \
		--tensor-parallel-size "${VLLM_TP_SIZE:-1}" \
		"${EXTRA_ARGS[@]}"
	echo "✅ vLLM listening on http://localhost:${PORT}"

# Tail vLLM logs (defaults to follow mode)
vllm-logs follow="true":
	#!/usr/bin/env bash
	set -euo pipefail
	CONTAINER="${VLLM_CONTAINER_NAME:-vllm-server}"
	if [[ "{{follow}}" == "true" ]]; then
		docker logs -f "$CONTAINER"
	else
		docker logs "$CONTAINER"
	fi

# Launch mistral.rs OpenAI-compatible server against a cached local model.
mistralrs-up model_id="mistralai/Mistral-7B-Instruct-v0.3" model_name="mistral-local" port="1234":
	#!/usr/bin/env bash
	set -euo pipefail
	MODEL_ID="{{model_id}}"
	MODEL_NAME="{{model_name}}"
	PORT="{{port}}"
	echo "🚀 starting mistralrs-server on :${PORT} with ${MODEL_ID}"
	mistralrs-server \
		--port "${PORT}" \
		--served-model-name "${MODEL_NAME}" \
		--hf-model-id "${MODEL_ID}"

# Smoke test local mistral.rs chat endpoint.
mistralrs-chat prompt="hello from b00t" port="1234" model_name="mistral-local":
	#!/usr/bin/env bash
	set -euo pipefail
	PROMPT="{{prompt}}"
	PORT="{{port}}"
	MODEL_NAME="{{model_name}}"
	curl -fsS \
		-H 'Content-Type: application/json' \
		-d "$(jq -nc --arg m "$MODEL_NAME" --arg p "$PROMPT" '{model:$m,messages:[{role:"user",content:$p}],max_tokens:120,temperature:0.2}')" \
		"http://127.0.0.1:${PORT}/v1/chat/completions" | jq -r '.choices[0].message.content // empty'
