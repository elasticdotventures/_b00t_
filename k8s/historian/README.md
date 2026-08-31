# b00t-historian on vultr1's k0s — issue #1229

**Status: NOT APPLIED.** Everything in this directory (and
`../../Dockerfile.historian`) is a reviewable artifact only. No image has
been built or pushed, and nothing has been copied to vultr1 or applied to
its k0s cluster. This session had no SSH/kubectl access to
`vultr1.v.promptexecution.com` (confirmed: SSH times out from the box this
was written on) — actual deployment is a manual follow-up step for whoever
has access to that box.

## What this is

Deploys `b00t-historian` (`b00t-cli/src/bin/b00t-historian.rs`) as a real
k0s workload on vultr1/b00t-node, per #1229: a hive-wide coordinator
subscribed to `souls.>`, `vultr.>` (both always-on, hardcoded in the
binary's `run` loop) and `b00t.hive.mesh.>` (this deployment's one
`--subject` flag), instead of the current interim Python port on `sm3lly`
scoped only to `hive.sm3ll-fung1.>`.

Files:
- `../../Dockerfile.historian` — multi-stage build (cargo-chef, mirrors
  `Dockerfile.b00t-cli`'s structure) producing a small `debian:12-slim`
  runtime image with just the `b00t-historian` binary.
- `configmap.yaml` — `NATS_URL` + `HISTORIAN_SUBJECT`, no secrets.
- `secret.example.yaml` — template for `NATS_USER`/`NATS_PASSWORD`. Copy to
  `secret.yaml`, fill in, **do not commit `secret.yaml`**. Read the
  comments in that file first — there's a known gap in the hub NATS
  server's current auth config that this alone doesn't fix (see below).
- `deployment.yaml` — single-replica Deployment, `hostNetwork: true`,
  hostPath volume for the durable NDJSON archive.

## Known gap: hub NATS auth (read before filling in secret.yaml)

The k0s-managed NATS server on vultr1 (`pods/nats/nats-pod-configured.yaml`
in the infrastructure repo, applied via `podman play kube`, client port
hostPort 4222) runs in **operator/JWT multi-tenant auth mode**
(`auth_required: true`), with only the `SYS` account preloaded in its
resolver. `b00t-historian` only supports plain `--nats-user`/
`--nats-password` auth (no NKey/creds-file path) — pointed at that server
as configured today, it will hit `-ERR 'Authorization Violation'`, the
exact same documented gap that already blocks the `dapr` `nats-pubsub`
component on this box (see the `b00t-daprd.service` comment in
`terraform/b00t/cloud-init/b00t-node.yaml.tpl`).

This PR ships the env-var plumbing (`NATS_USER`/`NATS_PASSWORD` ->
container, `optional: true` so the Deployment still starts and fails
visibly in `kubectl logs` rather than crash-looping on a missing Secret)
so that once a real plain-auth account/user is minted for this workload —
e.g. the same way `capability-forge` mints accounts, or a plain-auth
account added to `nats-pod-configured.yaml` — wiring it up is just filling
in `secret.yaml`. Actually minting that credential and confirming the
connection works is explicitly **not** done by this PR; it's part of the
manual apply step below.

## Build + push (not yet done)

```sh
# from the repo root
docker build -f Dockerfile.historian \
  --build-arg BUILD_VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1) \
  --build-arg BUILD_COMMIT=$(git rev-parse --short HEAD) \
  --build-arg BUILD_DATE=$(date -u +%Y-%m-%dT%H:%M:%SZ) \
  -t ghcr.io/elasticdotventures/b00t-historian:latest .

docker push ghcr.io/elasticdotventures/b00t-historian:latest
```

(Mirrors `.github/workflows/b00t-cli-container.yml`'s existing
`ghcr.io`/`elasticdotventures` convention. Wiring an equivalent CI workflow
for this image — auto-build on `Dockerfile.historian`/`b00t-cli/**`
changes — is a reasonable follow-on but out of scope here: a working,
reviewable manifest is the priority for this PR, not a new CI pipeline.)

This session did not attempt this build: no scoped need to spend the CI
minutes/registry push from here, and `docker build`/`podman build` were not
run in this session at all — see "Validation performed" below for what was
actually verified instead.

## Deploy to vultr1 (manual — requires access this session does not have)

1. Build + push the image (above), from a machine/CI runner that has
   registry credentials.
2. `scp` or otherwise copy this directory's manifests to vultr1:
   ```sh
   scp k8s/historian/{configmap.yaml,deployment.yaml,secret.yaml} \
     vultr1.v.promptexecution.com:/tmp/historian/
   ```
   (`secret.yaml` here is the filled-in copy of `secret.example.yaml` —
   never the example itself with placeholder values, and never committed.)
3. On vultr1, place them under k0s's static-manifest directory so the
   stack controller applies **and keeps re-applying** them (the same
   reason the OIDC discovery RBAC binding lives under
   `/var/lib/k0s/manifests/oidc-discovery-rbac/` instead of being
   `kubectl apply`'d once — confirmed by live testing that a one-off apply
   does not survive a k0s restart on this box):
   ```sh
   sudo mkdir -p /var/lib/k0s/manifests/historian
   sudo cp /tmp/historian/*.yaml /var/lib/k0s/manifests/historian/
   ```
4. Confirm the Secret's NATS account actually works before expecting
   traffic — see "Known gap" above. Watch `kubectl -n default logs deploy/
   b00t-historian` for `Authorization Violation` vs. a clean subscribe.
5. Verify against #1229's acceptance criteria (discovery query from a
   different LAN leaf gets a reply from a peer connected via a different
   leaf, etc.) — this is real end-to-end verification that requires the
   live mesh and is explicitly not something this PR alone can confirm.

## Validation performed in this PR (no cluster/build access)

- `python3 -c "import yaml; list(yaml.safe_load_all(open(f)))"` for every
  `.yaml` file in this directory — see the PR description for the exact
  output.
- `podman build -f Dockerfile.historian --target planner .` (`podman`
  available in this session, `docker` on this box is itself a podman
  shim) — ran successfully: every `COPY` path resolves and
  `cargo chef prepare --recipe-path recipe.json` completed without error,
  confirming the workspace-member COPY list is correct and the workspace
  resolves cleanly through cargo-chef's dependency planner.
- The remaining stages (`cargo chef cook --release` compiling ~40 workspace
  crates including native deps like `embed_anything`/`esaxx-rs`, then
  `cargo install --path b00t-cli --bin b00t-historian`) were **not** run —
  a full release compile of this workspace is minutes-to-tens-of-minutes
  even with warm caches, and this session had no registry to push a
  resulting image to anyway. Those stages were checked by eye against the
  already-working `Dockerfile.b00t-cli` instead (same base images, same
  COPY set, same `cargo install --path b00t-cli` entry point — only the
  `--bin` target and final runtime stage differ). Run the full
  `docker build`/`podman build` (no `--target`) as part of the manual
  deploy step above before trusting the image boots.
