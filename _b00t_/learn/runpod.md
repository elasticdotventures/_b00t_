---
endpoint template: EndpointCreateInput.template_id is required (not Option). Env vars bake into the RunPod template via console first — cannot be set at API deploy time.

---
docker_args silence: CreateOnDemandPodRequest.docker_args was silently dropped from REST payload in v0.1.30 — pod launched without startup CMD. Fix: PR agentsea/runpod.rs#2; pin PromptExecution/runpod.rs@fix/b00t-support-public-ip until merged.

---
GPU type enum: RunPod REST API requires exact GPU enum strings. Use full NVIDIA prefix: 'NVIDIA A100-SXM4-80GB' not 'A100 SXM4 80GB'. Check valid values via b00t-cli runpod ping.
