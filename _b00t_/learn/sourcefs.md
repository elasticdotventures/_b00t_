---
moto: "SourceFS is a proprietary virtual filesystem (Rust) that accelerates Android AOSP checkout+build from 3 hours to 5 minutes. Materializes files on demand — only the ~0.01% of codebase touched by a change is ever downloaded. Build steps executed in lightweight sandboxes; matching prior records are replayed, not re-executed. Covers not just compilation but linking, packaging, docs — 99%+ of build steps."

principle: "The pattern b00t can apply without SourceFS: Docker layer caching for SDK/NDK/templates (don't re-download), Gradle configuration cache + build cache (don't recompile unchanged modules), volume-mounted Gradle cache across container runs. Same principle: only rebuild what changed."

benchmarks: "On AWS: AOSP 16 checkout (41min → 1min), build (2h54min → 5min). Cost: $2.98 → $0.09 (97% reduction). Disk: 83x less. Scales linearly — 96 vCPU reaches 5min build time (diminishing returns after 32 vCPU)."

relevance: "For Oreo's app4dog: Gradle build caching in Dockerfile.android already applies this pattern. The 3+ hour Android build → 15-min target is the same principle at smaller scale. When b00t scales to multiple Android targets, SourceFS or equivalent build replay becomes essential."

sourcefs-vs: "SourceFS outperforms Bazel/Buck2 migration (no migration needed), compiler wrappers like REClient/Goma (cover <50% of build steps), and ccache (compilation only). It replays ALL build steps — linking, packaging, docs, code generation — not just C/C++ compilation."

# b00t:map v1
# summary: SourceFS — virtual filesystem for Android AOSP build acceleration (3h→5min, 97% cost reduction, Rust)
# tags: sourcefs, android, build, caching, virtual-filesystem, aosp, gradle, performance
# tier: frontier
# cmds: just android-container-build (Gradle caching applied)
# complexity: 7
