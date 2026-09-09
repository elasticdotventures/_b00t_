# Lazy-pull image snapshotters: eStargz vs SOCI vs Nydus — evaluation

**Date:** 2026-09-09
**Context:** plan gleaming-jingling-nygaard, build-plane leverage #5 (image-pull time).
**Question:** should the b00t build plane (and the future PromptExecution
"1000s of cold-standby services" fleet) adopt a lazy-pull snapshotter, and which?
**Operator prior:** "SOCI is interesting — open to evidence."

## TL;DR

1. **Now, on the VM-based dstack plane: adopt none of them.** All three are
   *containerd remote-snapshotter plugins*; dstack provisions a cloud VM and
   runs the workload with **dockerd**, with no hook to swap in a custom
   containerd + remote snapshotter. Instead kill the pull at the GCP layer:
   bake `b00t-build:latest` into a **GCE custom machine image / disk snapshot**
   (refreshed by the existing weekly `b00t-build-image.yml`). Boot-from-image =
   pull is a no-op. Same "seconds not minutes to container-ready" payoff,
   zero new runtime moving parts, compatible today.
2. **Later, if the plane or the services fleet moves to `backend: kubernetes`
   / bare-metal containerd: SOCI.** Non-invasive (no image conversion), AR-
   compatible, no extra long-lived daemon beyond the snapshotter, and its
   prefetch-heavy behaviour is the right default for CI/batch. Matches the
   operator's instinct.
3. **Nydus** is the pick *only* at true fleet scale (50+ nodes pulling
   overlapping images) where its native Dragonfly P2P egress savings outweigh
   its ops cost (nydusd, RAFS conversion pipeline, EROFS/fscache tuning).
4. **eStargz**: skip. Pays SOCI's conversion cost with worse prefetch
   semantics.

## The decisive constraint

All three plug into **containerd** as a *remote snapshotter* and require
configuring/replacing the snapshotter on every node. They are **not** OCI
runtimes and do **not** work under plain `dockerd` (Docker's containerd image
store, Engine 25+, still does not expose remote snapshotters).

dstack's GCP backend = provision a GCE VM → `docker run` the config's `image:`.
There is no dstack setting to install containerd + `soci-snapshotter` /
`nydus-snapshotter` / `stargz-snapshotter` on that VM and point the runtime at
it. So on the current plane the snapshotter question is **moot until** one of:

- dstack `backend: kubernetes` (we own the node image + containerd config), or
- dstack on bare-metal / on-prem with our own containerd, or
- a future dstack feature to bring-your-own container runtime.

Until then, the GCE machine-image approach (TL;DR #1) is the pragmatic
equivalent and needs no upstream change.

## Comparison (for when containerd is ours to configure)

| Dimension | eStargz | SOCI | Nydus |
|---|---|---|---|
| Image conversion | **Yes** — recompress every layer at build (~2% larger blobs) | **No** — original image untouched; external `zTOC` index as a separate artifact | **Yes** — RAFS v6 format; *or* a tiny `nydus-zran` artifact generated from an existing OCI image without full conversion |
| Referrers API needed | No (TOC is in-layer) | Yes — index attached via OCI 1.1 Referrers; SOCI ≥0.7 has a derived-tag fallback | No (RAFS/zran pushed as a normal OCI artifact) |
| GCP Artifact Registry | ✅ plain image | ✅ AR has Referrers GA (2024); tag fallback otherwise | ✅ stores the extra artifact like any blob |
| Daemon | FUSE (`stargz-snapshotter`) | FUSE (`soci-snapshotter`) | `nydusd` FUSE **or** in-kernel **EROFS + fscache**, no FUSE (Linux ≥5.19; GCE Ubuntu 24.04 = 6.8 ✅) |
| Prefetch model | defers ~everything to first read (purest laziness) | span-based; fetches most of the image before Ready | explicit prefetch list + chunk-on-demand |
| Native P2P / Dragonfly | partial | none | **native** — Nydus *is* the Dragonfly image service |
| Other backends | registry only | registry only | registry, S3, OSS, NAS |
| Ops complexity | low | low–medium | **high** (nydusd, RAFS toolchain, conversion in CI, cache tuning) |
| Project health (2026) | containerd sub-project, stable, slow cadence | containerd sub-project, AWS-driven, very active, ECR-first | containerd sub-project + Dragonfly/CNCF, most feature-rich, most complex |

## Why SOCI for the k8s/bare-metal phase

- **Non-invasive is the scale property that matters.** At "1000s of services"
  you do not want a format-conversion pipeline gating every image push. SOCI
  leaves every image a normal OCI image, usable unchanged everywhere it isn't
  wanted; the `zTOC` is a CI side-artifact (`soci create` + `soci push`).
- **AR works** (Referrers GA + tag fallback).
- **No RAFS toolchain, no `nydusd`** — one snapshotter binary per node.
- **Prefetch-heavy suits batch/CI.** You want the image resident fast and
  predictably, not death-by-FUSE-read mid-`cargo build`. eStargz's pure
  laziness is the wrong default here.
- Nydus's raw-perf / P2P edge is real but only pays for its complexity at
  fleet scale — revisit it *specifically* for the 1000-node fleet, paired with
  Dragonfly, where registry egress dominates cost.

## ⚠️ Universal caveat — deferred cost, deferred failure

Lazy pull moves latency and failure from *pull time* to *first-read time*
(arXiv 2608.19412, "The Lazy Pod That Lies"; zmalik.dev deep-dive). A registry
blip or a cold read stalls the workload mid-run instead of failing fast at
schedule time. For CI that's a flake; for a latency-sensitive service it's an
SLO risk. Whichever is adopted: enable aggressive background prefetch and treat
the registry as a hard critical-path dependency (mirror / P2P / pull-through
cache).

## Recommendation ledger

| Phase | Plane shape | Action |
|---|---|---|
| Now | dstack + GCE VMs + dockerd | GCE machine-image with `b00t-build` pre-baked; **no snapshotter** |
| Next | dstack `backend: kubernetes` or bare-metal containerd | **SOCI** — `soci create/push` in `b00t-build-image.yml`, `soci-snapshotter` on nodes |
| Fleet | 50+ nodes, overlapping images, egress-bound | **Nydus + Dragonfly** P2P; accept the ops cost |
| — | any | eStargz — not chosen |

## Sources

- https://blog.zmalik.dev/p/lazy-pulling-container-images-a-deep
- https://github.com/containerd/nydus-snapshotter
- https://github.com/containerd/stargz-snapshotter
- https://github.com/dragonflyoss/nydus/blob/master/docs/nydus-zran.md
- https://www.buildbuddy.io/blog/image-streaming/
- https://sreake.com/blog/container-lazy-pull-soci-snapshotter/
- https://arxiv.org/html/2608.19412 — deferred cost / failure semantics
- https://dstack.ai/docs/concepts/dev-environments/ — dstack container model
