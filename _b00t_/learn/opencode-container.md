---
moto: "OpenCode container image: ghcr.io/anomalyco/opencode (deprecated) → ghcr.io/sst/opencode (current). Repo moved from anomalco/opencode to sst/opencode. b00t fork at vendor/opencode-b00t (PromptExecution/opencode-b00t) tracked upstream dev branch."

sync: "Josh pattern: fork → track upstream → merge regularly → keep b00t config patches on top. Current: synced with sst/opencode upstream/dev (139 files, 5012 insertions). One b00t commit on top: reviewer sub-agents MECE+TRIZ+Eureka review with verdict contract."

config: "OpenCode config lives in opencode.json (project or global). b00t modifications: plugin list, command registrations, agent configuration. These are user config, not fork patches — they live in the target project, not the vendor fork."

run: "podman run -it --rm ghcr.io/anomalyco/opencode — pulls the container image. For b00t integration: mount config dir, mount workspace, set API keys via env vars."

# b00t:map v1
# summary: OpenCode container + fork sync — ghcr.io/sst/opencode, vendor/opencode-b00t synced with upstream/dev
# tags: opencode, container, fork, sync, josh-pattern, b00t, config
# tier: ch0nky
# cmds: podman run -it --rm ghcr.io/anomalyco/opencode, cd vendor/opencode-b00t && git merge upstream/dev
# complexity: 5
