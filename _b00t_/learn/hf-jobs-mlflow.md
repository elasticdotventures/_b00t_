# HF Jobs + MLflow Tracking

## b00t:map v1
# summary: MLflow on LAN IP is NOT reachable from HF Jobs cloud workers; mlflow not in default unsloth image
# tags: hf-jobs, mlflow, networking, cloud
# tier: frontier
# cmds: report_to: "none"   # for cloud runs
# complexity: 3

## LFMF: MLflow LAN IP timeouts HF Jobs

**Lesson**: `mlflow_tracking_uri: "http://192.168.1.137:30803"` is a LAN IP (sm3lly NodePort). HF Jobs cloud workers are on HF's cloud infrastructure — they CANNOT route to private LAN addresses.

**Symptom**: Job fails after model load with `ConnectTimeoutError` after 120s.

**Fix**: Use `report_to: "none"` in cloud training configs. Monitor via `hf jobs logs <id>`.

**For local k8s runs**: MLflow IS reachable — keep `report_to: "mlflow"` in local configs.

**Pattern**:
- cloud config: `report_to: "none"`
- local config: `report_to: "mlflow"`, `mlflow_tracking_uri: "http://192.168.1.137:30803"`

---
MLflow not installed in docker.io/unsloth/unsloth:latest (ships vLLM variant). Setting report_to:mlflow without mlflow package causes RuntimeError at SFTTrainer.__init__ before any training step. Fix: bake mlflow into custom training image via uv pip install, or set report_to:none.
