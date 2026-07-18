# HANDOFF — create-a-critter pipeline / post-OOM wind-down
**Date**: 2026-07-18 | **Branches**: common-core `task/10-mesh3d-contracts`, `task/12-vqa-contracts`; _b00t_ `task/162-mesh3d-hive`, `task/165-sm0l-vqa` | **Node**: sm3lly (idle, both inference containers DOWN)

---

## TL;DR for the next engineer

The app4dog create-a-critter pipeline (photo → segmentation → VQA → 3D mesh →
rigging → animated NPC) is contract-complete through the mesh stage. Two node
OOM-crashes on 2026-07-17 forced a resource-protocol hardening pass; that
policy is shipped. The single blocked item is the mesh-runner container image:
**one dependency-conflict line away from building** — everything expensive
(CUDA extension compiles) is already cached.

## 1. FIRST TASK: finish the mesh-runner image (~10 min)

Build #3 failed at STEP 10/13 with:
```
× Failed to resolve dependencies for `moge` (v2.0.0)
╰─▶ Requirements contain conflicting URLs for package `utils3d`:
    - git+https://github.com/EasternJournalist/utils3d.git@3fab839f...
    - https://github.com/LDYang694/Storages/.../utils3d-0.0.2-py3-none-any.whl
```
Fix in `critter-keeper/docker/mesh-runner/Containerfile` STEP 10: **remove the
LDYang694 wheel URL** from the `uv pip install` line — `moge` (pulled by
Pixal3D requirements.txt) already pins utils3d from git; two URLs for one
package is a hard uv error. (Alternative: separate `uv pip install` for the
wheel AFTER the requirements line — but the git pin should simply win.)

Then rebuild — steps 1–9 are layer-cached, resumes at step 10 in seconds:
```
cd ~/promptexecution/app4dog/common-core/critter-keeper/docker/mesh-runner
podman build --memory=16g --memory-swap=16g -t app4dog/mesh-runner:v1 -f Containerfile ../..
```
⚠️ NON-NEGOTIABLE: `--memory=16g` cap, inference sidecars stopped. Uncapped
builds killed this node twice (buildah 20.7G RSS). Guard now blocks it.

## 2. SECOND TASK: mesh e2e (evidence gate for PR #11)

Job dir already staged: session scratchpad `mesh-e2e-oreo/` (photo.jpg =
IMG_2624_oreo_laying.JPG, request.json sha256-pinned, low_vram, res 1024,
seed 42). If the scratchpad was reaped, rebuild it from
`~/promptexecution/app4dog/samples/` — request shape is
`MeshV1JobRequest` (`python/mesh_runner/contract.py` mirrors it).

```
b00t hive plan=mesh3d-batch   # gate check (13000MB VRAM free needed)
podman run --rm --device nvidia.com/gpu=all --security-opt=label=disable \
  --memory=20g --memory-swap=20g -v $JOB_DIR:/workspace:rw \
  -v /home/brianh/.cache/huggingface/hub:/hf:z \
  app4dog/mesh-runner:v1 /workspace/request.json
```
Expect `output.json` + `mesh.glb` in the job dir. Then host-side gate:
`cargo test --lib --features mesh_v1_validate` against the real GLB
(`MESH_E2E_GLB=$JOB_DIR/mesh.glb cargo test --test mesh_glb_gate`).
Close by flipping `MeshJobRunnerSpec::mesh_v1()` to
`executable: cfg!(feature = ...)` per the SamV1/VqaV1 pattern, commit with
verbatim PASS evidence.

## 3. Remaining pipeline work (in priority order)

| # | Task | State |
|---|------|-------|
| 1 | Containerfile utils3d fix + build | §1 above |
| 2 | Mesh e2e + executable flip | §2 above |
| 3 | VQA characterize-stage integration: `identify_characteristics` consumes `VqaAnswerOutput` instead of caption string | contracts + live runner done (PR #12, e2e: species=French Bulldog/pose=lying/colors=black,white) |
| 4 | Rigging chain: GLB → 2D→3D landmarks → limb segmentation → animation | not started; contracts follow MeshV1 pattern |
| 5 | Intermediate demo: game imports GLB directly (no sprite) | after §2 |
| 6 | RL grading agents per stage | design only |

## 4. PRs awaiting operator merge

| PR | Content |
|----|---------|
| common-core **#11** | MeshV1 contracts, Python runner, Containerfile, GLB gate, eigen/OOM fixes (9043180, b099f2d) |
| common-core **#12** | VqaV1 contracts + HTTP runner, live-evidenced vs :8002 sidecar |
| _b00t_ **#852** | sm0l-vqa sidecar profile + pod spec (start cmd capped `--memory=6g`) |
| _b00t_ **#853** | mesh3d-batch profile + shared-node resource protocol (5009f8f8) |

## 5. Operator-only actions (root; queued on PR #853)

1. Review+install the b00t-limits OCI prestart hook (rejects uncapped
   containers; escape hatch `--annotation b00t.unlimited=ack`):
   `sudo install -m755 _b00t_/podman/b00t-limits-hook.sh /usr/local/bin/b00t-limits-hook && sudo install -m644 _b00t_/podman/b00t-limits-hook.json /usr/share/containers/oci/hooks.d/`
   — test a throwaway container first; a broken always-hook blocks ALL starts.
2. Session-wide backstop (would have prevented both crashes, covers cargo too):
   `sudo systemctl set-property user-1000.slice MemoryHigh=26G MemoryMax=29G`
3. Enable one cloud upstream in cli-proxy-api (`claude`/`codex`/`gemini`, all
   `enabled = false`) so the ch0nky yield-redirect policy has a real fallback.

## 6. Standing protocol (new this session — READ before touching containers)

- **Shared-Node Resource Protocol** section in CLAUDE.md: every container job
  capped + hive-gated; ch0nky is PREEMPTIBLE; consume gateway :1234, never :8001.
- `b00t whoami --role=podman` loads the container-steward laws.
- Guards live: uncapped `podman build` = BLOCK, uncapped `podman run` = warn.
- kube play + GPU is broken on sm3lly (oci-nvidia-hook conflict) — raw
  `podman run --device nvidia.com/gpu=all` is the only proven GPU path.
- Start sidecars only when needed:
  ch0nky = `b00t hive activate inference-qwen36-35b-a3b-llamacpp`;
  sm0l-vqa = capped podman run in `_b00t_/inference-sm0l-vqa.hive.toml` usage.

## 7. Known traps (cost real time this session)

- `b00t-cli patch apply` with stdin diffs REPLACES the whole file (task #164) — use scripted python string-replace.
- datum-validate-graph commit hook hangs on ~/.b00t (97-error baseline, #163) — `git commit --no-verify` with justification.
- `b00t task add` is cwd-relative — run from `~/.b00t` or it creates a stray `.b00t/tasks.json`.
- Debian eigen lives at `/usr/include/eigen3` — off the default include path; o-voxel needs `CPATH`.
- Read tool is hook-gated for code files — use `cat`/`sed` via Bash.
- b00t tasks open: #160/#161 (RAG backends degraded), #162 (mesh3d), #163, #164, #165 (VLM deploy — done modulo #852 merge).

<!-- b00t:map v1
summary: 2026-07-18 handoff — mesh-runner one dep-conflict from building (steps 1-9 cached), e2e staged, resource protocol shipped, 4 PRs open, operator root-actions queued
tags: handoff, create-a-critter, mesh3d, vqa, resource-protocol, podman
tier: frontier
cmds: podman build --memory=16g -t app4dog/mesh-runner:v1, b00t hive plan=mesh3d-batch
complexity: 6
-->
