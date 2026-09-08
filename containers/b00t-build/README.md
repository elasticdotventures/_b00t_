# b00t-build image

Prebaked build-box image for the dstack build plane (plan
gleaming-jingling-nygaard). `ghcr.io/elasticdotventures/ci-occt` turned out to
be **empty except `git`** — so the dev-env `init:` was installing the entire
toolchain on every provision (~6–10 min). This image bakes it.

## Contents

Ubuntu 24.04 + `build-essential cmake clang lld pkg-config libssl-dev z3
protobuf-compiler` + rustup stable + prebuilt `sccache` / `cargo-nextest` +
`gcsfuse`.

## Build / push

```sh
cd containers/b00t-build
IMG=australia-southeast1-docker.pkg.dev/promptexecution/b00t/b00t-build:latest
podman build -t "$IMG" -f Containerfile .
printf '%s' "$GITHUB_TOKEN" | podman login ... # (AR: gcloud auth configure-docker australia-southeast1-docker.pkg.dev)
podman push "$IMG"
```

CI: add a job to `.github/workflows/` that rebuilds weekly + on this dir's
changes, pushing to the same AR path.

## Wiring

Point `dev-env/b00t-build.dev-environment.yaml` `image:` at `$IMG` and trim its
`init:` to just: `cache-up.sh`, the `_b00t_` clone + submodules, `mkdir` the
sccache/target dirs. Keep `privileged: true` (gcsfuse needs `/dev/fuse`).

## Not baked (deliberately)

The `_b00t_` checkout + `vendor/*` — those change constantly and are cloned in
`init:` onto local disk. Only the slow-moving toolchain is baked.
