---
COPY vs ADD best practices: Use COPY for local files and ADD only for URLs or archives that need auto-extraction

---
build-arg version bump does not guarantee fresh download: Bumping an ARG (e.g. ARG TOOL_VERSION=x.y.z) in a Dockerfile/Containerfile does not guarantee buildx re-fetches the artifact — layer caching can silently keep the old binary even with a cache-bust comment. Verify by checking the running binary's actual reported version inside the built image, not the Dockerfile ARG value.

---
rootful socket is a security anti-pattern: The classic Docker daemon requires a rootful background socket (/var/run/docker.sock) that grants root-equivalent access to anyone who can reach it — this fails security/cyber review in hardened environments. Podman is a drop-in CLI-compatible replacement that runs rootless by default (no privileged daemon at all) and needs no docker binary present; prefer podman for all local container work, and treat any script hardcoding 'docker' as a signal to update it to call podman directly.
