---
COPY vs ADD best practices: Use COPY for local files and ADD only for URLs or archives that need auto-extraction

---
build-arg version bump does not guarantee fresh download: Bumping an ARG (e.g. ARG TOOL_VERSION=x.y.z) in a Dockerfile/Containerfile does not guarantee buildx re-fetches the artifact — layer caching can silently keep the old binary even with a cache-bust comment. Verify by checking the running binary's actual reported version inside the built image, not the Dockerfile ARG value.
