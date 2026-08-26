"""
Full from-scratch bootstrap for b00t-node (Vultr VPS, Debian 13/trixie),
reconstructing the souls/NATS stack exactly as it exists in production as of
2026-08-25. Idempotent — safe to re-run against an already-configured host.
This is the memoized answer to "how do we rebuild this node without an LLM."

Deliberately NOT reproduced here (known-broken cruft — see the "DEPLOYED
2026-08-25" and earlier entries in _b00t_/datums/PROVIDER-VULTR.provider.tomllmd
for the full incident history):
  - b00t-daprd.service            — crash-looping; Dapr's pubsub.jetstream
    account was never actually provisioned, souls uses plain NATS+JetStream
    instead (see the NATS auth section below)
  - b00t-pingap.service, b00t-maintenance.service — unrelated to souls; add a
    separate deploy_*.py for those if/when they need to be reproducible too

b00t-capability-forge.service WAS in this exclusion list (crash-looping,
26000+ restarts) — fixed and reproduced here as of 2026-08-26, see section
7 below / _b00t_#1154.

The "operator wants b00t-server dogfooded onto this node" follow-up (also
present since the earliest entries in this datum) is likewise resolved and
reproduced here — see section 8 below.

Usage:
    pyinfra nats/pyinfra/inventory.py nats/pyinfra/deploy_b00t_node.py \\
        --data nats_password=$(openssl rand -hex 24)

    # Dry run first (reports planned changes, does not apply them — NOTE:
    # the local cargo build step below still runs during a dry run, since
    # it happens on the controller machine, not the target host):
    pyinfra --dry nats/pyinfra/inventory.py nats/pyinfra/deploy_b00t_node.py \\
        --data nats_password=placeholder

Never pass the LIVE production nats_password on the CLI in a way that lands
in shell history / process listings you don't control — generate a fresh one
per rebuild (openssl rand -hex 24), same as vultr-node-setup.sh's own
LEAF_PASSWORD pattern. This script does not read or write the live secret;
it is not embedded anywhere in this repo.
"""

from pyinfra import host, local
from pyinfra.facts.files import File
from pyinfra.operations import apt, files, server, systemd

nats_password = host.data.get("nats_password")
if not nats_password:
    raise ValueError(
        "pass --data nats_password=<secret> (e.g. $(openssl rand -hex 24)) — "
        "never hardcode the live credential in this repo"
    )

# capability-forge's NATS user (section 7) — its password is needed as
# early as section 5's NATS pod config render, so it's fetched up here
# alongside nats_password rather than next to the rest of section 7.
# Required --data: capforge_nats_password, capforge_account_seed,
# capforge_account_pubkey (generate the account seed/pubkey ONCE with a
# throwaway `nkeys::KeyPair::new_account()` — e.g. via `cargo run` on a
# one-off scratch binary — and pass the SAME pair on every re-run; a fresh
# pair on each run would invalidate every capability grant this service
# has already issued). Optional: openai_api_key (escalation judge; fails
# closed without it — base-tier skills work fine either way).
capforge_nats_password = host.data.get("capforge_nats_password")
capforge_account_seed = host.data.get("capforge_account_seed")
capforge_account_pubkey = host.data.get("capforge_account_pubkey")
if not (capforge_nats_password and capforge_account_seed and capforge_account_pubkey):
    raise ValueError(
        "pass --data capforge_nats_password=<secret> --data capforge_account_seed=<nkeys seed> "
        "--data capforge_account_pubkey=<nkeys pubkey> — generate the seed/pubkey ONCE and reuse "
        "them on every re-run, never regenerate"
    )

# ─── 1. Base packages ──────────────────────────────────────────────────────
apt.packages(
    name="Install podman + iptables + curl",
    packages=["podman", "iptables", "curl"],
    update=True,
)

# ─── 2. k0s CNI isolation — MUST land before k0s ever starts ───────────────
# Root cause of the CrashLoopBackOff incident (2026-08-22 through
# 2026-08-24): k0s's CNI config and Podman's both dropped into the shared
# /etc/cni/net.d, and new pod sandboxes landed on Podman's 10.88.0.0/16
# instead of k0s's 10.244.0.0/24, so coredns couldn't reach the apiserver
# ClusterIP. On a *fresh* host this file lands first, so k0s's containerd
# writes its own conflist straight into the exclusive dir from the start —
# no post-hoc "copy the file out, delete the old one" surgery required.
files.directory(name="Create /etc/k0s/containerd.d", path="/etc/k0s/containerd.d")
files.put(
    name="Give k0s containerd its own exclusive CNI conf_dir",
    src="files/10-cni.toml",
    dest="/etc/k0s/containerd.d/10-cni.toml",
)

# ─── 3. k0s (single-node control plane) ────────────────────────────────────
if not host.get_fact(File, path="/usr/local/bin/k0s"):
    server.shell(
        name="Install k0s",
        commands=["curl -sSLf https://get.k0s.sh | sh"],
    )

if not host.get_fact(File, path="/etc/systemd/system/k0scontroller.service"):
    server.shell(
        name="Register k0s as a single-node controller",
        commands=["k0s install controller --single"],
    )

systemd.service(
    name="Enable + start k0scontroller",
    service="k0scontroller.service",
    running=True,
    enabled=True,
)

# ─── 4. FORWARD-chain ACCEPT rules for podman0 ─────────────────────────────
# k0s's kube-router rewrites the FORWARD chain (default-DROP) on every
# k0scontroller start, with no rule for podman0 — without this, ANY podman
# host-published port fails "No route to host", not just NATS's. A oneshot
# unit (not a one-off manual iptables call) survives every future restart.
files.put(
    name="Upload FORWARD-chain rules script",
    src="files/b00t-podman-forward-rules.sh",
    dest="/usr/local/bin/b00t-podman-forward-rules.sh",
    mode="755",
)
files.put(
    name="Install b00t-podman-forward-rules.service",
    src="files/b00t-podman-forward-rules.service",
    dest="/etc/systemd/system/b00t-podman-forward-rules.service",
)
systemd.service(
    name="Enable + run b00t-podman-forward-rules",
    service="b00t-podman-forward-rules.service",
    running=True,
    enabled=True,
    daemon_reload=True,
)

# 🤓 also worth checking on ANY podman host in this hive with a k0s/kube-router
# neighbor, not just this one: `ip link show` for a second interface claiming
# podman0's subnet (e.g. a vestigial cni-podman0 from an old CNI-plugin-based
# podman config sourced from a stale /etc/cni/net.d/*.conflist) — that also
# produces "No route to host" and is NOT a firewall problem. Confirm via
# `podman network inspect podman` that nothing but podman0/netavark is live;
# `ip link delete <dead-iface>` + remove the stale conflist if one exists.
# Not codified as an operation here because it's a diagnostic check, not a
# deterministic provisioning step — a fresh host with no CNI-plugin podman
# history shouldn't hit it, per this deploy's own ordering (CNI isolation
# lands before k0s ever starts).

# ─── 5. NATS (podman play kube) ────────────────────────────────────────────
files.directory(name="Create /opt/b00t", path="/opt/b00t")
files.template(
    name="Render NATS pod + ConfigMap (simple user/pass auth)",
    src="templates/nats-pod-configured.yaml.j2",
    dest="/opt/b00t/nats-pod-configured.yaml",
    nats_password=nats_password,
    capforge_nats_password=capforge_nats_password,
)
files.put(
    name="Install b00t-nats.service",
    src="files/b00t-nats.service",
    dest="/etc/systemd/system/b00t-nats.service",
)
systemd.service(
    name="Enable + start b00t-nats",
    service="b00t-nats.service",
    running=True,
    enabled=True,
    daemon_reload=True,
)

# ─── 6. b00t-historian + b00t-forge-kv ─────────────────────────────────────
# Built locally as static musl binaries (not on the target — these nodes are
# provisioned lean, no Rust toolchain) and uploaded, matching the pattern
# already proven for b00t-forge-kv (nats/vultr-forge-kv-deploy.sh).
#
# 🤓 SUPERSEDED 2026-08-25 (_b00t_#1149): this used to require building
# inside a rust:alpine container (podman) — plain musl-tools has no C++
# support, and esaxx-rs (a transitive dep via tokenizers, pulled in by
# model2vec-rs's own default features) needed one to compile. Root-caused
# and fixed at the source (model2vec-rs default-features=false — see
# vendor/embed-anything-b00t/rust/Cargo.toml), so a plain
# `rustup target add x86_64-unknown-linux-musl` + host musl-gcc now builds
# cleanly, no container needed. The old container approach is left below,
# commented out, in case a *future* dependency reintroduces a C++
# requirement — flip back to it rather than rediscovering the Alpine fix.
#
# REPO_ROOT = local.shell("git rev-parse --show-toplevel").strip()
# CONTAINER_IMAGE = "docker.io/library/rust:1-alpine"
#
# local.shell(
#     f"podman run --rm --memory=8g --memory-swap=8g -v {REPO_ROOT}:/src:Z -w /src {CONTAINER_IMAGE} sh -c "
#     "'apk add --no-cache build-base perl linux-headers pkgconfig openssl-dev "
#     "&& cargo build --release --jobs 4 --bin b00t-historian -p b00t-cli "
#     "&& cargo build --release --jobs 4 -p b00t-forge-kv'"
# )
#
# for binary_name in ("b00t-historian", "b00t-forge-kv"):
#     files.put(
#         name=f"Upload {binary_name}",
#         src=f"{REPO_ROOT}/target/release/{binary_name}",
#         dest=f"/usr/local/bin/{binary_name}",
#         mode="755",
#         add_deploy_dir=False,
#     )

MUSL_TARGET = "x86_64-unknown-linux-musl"

local.shell(f"rustup target add {MUSL_TARGET}")

# Resolve the real cargo target dir via `cargo metadata` rather than
# assuming `<repo>/target` — the controller machine may set CARGO_TARGET_DIR
# (e.g. a shared build-cache dir) via .cargo/config.toml or its shell env.
TARGET_DIR = local.shell(
    "cargo metadata --no-deps --format-version=1 "
    "| python3 -c \"import json,sys; print(json.load(sys.stdin)['target_directory'])\""
).strip()

local.shell(
    f"cargo build --release --target {MUSL_TARGET} --jobs 4 --bin b00t-historian -p b00t-cli "
    f"&& cargo build --release --target {MUSL_TARGET} --jobs 4 -p b00t-forge-kv"
)

for binary_name in ("b00t-historian", "b00t-forge-kv"):
    files.put(
        name=f"Upload {binary_name}",
        src=f"{TARGET_DIR}/{MUSL_TARGET}/release/{binary_name}",
        dest=f"/usr/local/bin/{binary_name}",
        mode="755",
        add_deploy_dir=False,
    )

files.template(
    name="Write /etc/b00t-historian.env (0600)",
    src="templates/b00t-historian.env.j2",
    dest="/etc/b00t-historian.env",
    mode="600",
    nats_password=nats_password,
)
files.put(
    name="Install b00t-historian.service",
    src="files/b00t-historian.service",
    dest="/etc/systemd/system/b00t-historian.service",
)
files.put(
    name="Install b00t-forge-kv.service",
    src="files/b00t-forge-kv.service",
    dest="/etc/systemd/system/b00t-forge-kv.service",
)
systemd.service(
    name="Enable + start b00t-historian",
    service="b00t-historian.service",
    running=True,
    enabled=True,
    restarted=True,  # pick up a rebuilt binary / rotated password on re-run
    daemon_reload=True,
)
systemd.service(
    name="Enable + start b00t-forge-kv",
    service="b00t-forge-kv.service",
    running=True,
    enabled=True,
    daemon_reload=True,
)

# ─── 7. capability-forge ────────────────────────────────────────────────────
# 🤓 MEMOIZED 2026-08-26: was crash-looping in production (26000+ restarts,
# journalctl showing "cannot parse user JWT from the credentials file") —
# it only knew how to authenticate to NATS via a JWT/operator-mode creds
# file, but this node's NATS server runs plain username/password auth (see
# section 5 above / this file's nats_password). Fixed at the source in
# capability-forge's main.rs (_b00t_#1154): it now tries
# CAPFORGE_NATS_USER/CAPFORGE_NATS_PASSWORD first. Confirmed live: after
# this exact sequence (new NATS user via config reload, env file, rebuilt
# binary, service restart) it stayed up with 0 restarts.
#
# Unlike b00t-historian/b00t-forge-kv above, this does NOT build with a
# plain native musl cross-compile — openssl-sys (pulled in transitively,
# likely via the OpenAI judge client) needs pkg-config cross-compilation
# support that isn't set up on the controller, and fails outright:
# "Could not find directory of OpenSSL installation" /
# "pkg-config has not been configured to support cross-compilation."
# Falls back to the same rust:alpine container approach the historian
# build used before _b00t_#1149's esaxx-rs fix (see the commented-out
# block in section 6) — alpine's musl toolchain + openssl-dev sidesteps
# the cross-compile pkg-config problem entirely.
#
REPO_ROOT = local.shell("git rev-parse --show-toplevel").strip()
CONTAINER_IMAGE = "docker.io/library/rust:1-alpine"

local.shell(
    f"podman run --rm --memory=8g --memory-swap=8g -v {REPO_ROOT}:/src:Z -w /src {CONTAINER_IMAGE} sh -c "
    "'apk add --no-cache build-base perl linux-headers pkgconfig openssl-dev openssl-libs-static "
    "&& cargo build --release --jobs 4 --bin capability-forge -p capability-forge'"
)

files.put(
    name="Upload capability-forge",
    src=f"{REPO_ROOT}/target/release/capability-forge",
    dest="/usr/local/b00t/capability-forge",
    mode="755",
    add_deploy_dir=False,
)
files.link(
    name="Symlink /usr/local/bin/capability-forge -> /usr/local/b00t/capability-forge",
    path="/usr/local/bin/capability-forge",
    target="/usr/local/b00t/capability-forge",
)
files.template(
    name="Write /opt/b00t/capforge.env (0600)",
    src="templates/capforge.env.j2",
    dest="/opt/b00t/capforge.env",
    mode="600",
    capforge_nats_password=capforge_nats_password,
    capforge_account_seed=capforge_account_seed,
    capforge_account_pubkey=capforge_account_pubkey,
    openai_api_key=host.data.get("openai_api_key", ""),
)
files.put(
    name="Install b00t-capability-forge.service",
    src="files/b00t-capability-forge.service",
    dest="/etc/systemd/system/b00t-capability-forge.service",
)
systemd.service(
    name="Enable + start b00t-capability-forge",
    service="b00t-capability-forge.service",
    running=True,
    enabled=True,
    restarted=True,  # pick up a rebuilt binary / rotated password on re-run
    daemon_reload=True,
)

# ─── 8. b00t-server (OpenAI-compat gateway) ─────────────────────────────────
# 🤓 MEMOIZED 2026-08-26: reproduces the "dogfood b00t-server on this node"
# follow-up that's been open since the earliest MEMOIZED entries in
# PROVIDER-VULTR.provider.tomllmd. b00t-server is just b00t-mcp run with
# --http --llm — the same binary, no separate crate. Builds as a plain
# native musl cross-compile (unlike capability-forge in section 7): b00t-mcp
# depends on b00t-cli, which carries this workspace's `openssl-sys =
# { features = ["vendored"] }` pin — that vendored build compiles OpenSSL
# from C source using plain musl-gcc (pure C, no C++ needed, same reason
# the section-6 fix in _b00t_#1149 worked once esaxx-rs's C++ requirement
# was removed). capability-forge doesn't depend on b00t-cli, so that pin
# never reaches it, which is why it still needs the Alpine container.
#
# Backend discovery is entirely runtime-config-driven (see
# b00t-mcp/src/server_llm.rs's SoulConfig) — local backends (mistralrs,
# llama-cpp, vllm, qwen3-embed) are TCP-probed and silently skipped when
# nothing's listening (this node has no GPU; that's expected, not a
# degraded mode), falling through to the remote backend list. TELNYX_API_KEY
# is templated below because it's the one provider confirmed to have a
# real working credential anywhere in this hive as of 2026-08-26 — see
# this file's own section for how that was verified.
#
# Required --data: telnyx_api_key.
telnyx_api_key = host.data.get("telnyx_api_key")
if not telnyx_api_key:
    raise ValueError("pass --data telnyx_api_key=<secret> — no other remote LLM backend has a working credential in this hive yet")

local.shell(
    f"cargo build --release --target {MUSL_TARGET} --jobs 4 --bin b00t-mcp -p b00t-mcp"
)

files.put(
    name="Upload b00t-mcp",
    src=f"{TARGET_DIR}/{MUSL_TARGET}/release/b00t-mcp",
    dest="/usr/local/bin/b00t-mcp",
    mode="755",
    add_deploy_dir=False,
)
files.template(
    name="Write /opt/b00t/b00t-server.env (0600)",
    src="templates/b00t-server.env.j2",
    dest="/opt/b00t/b00t-server.env",
    mode="600",
    telnyx_api_key=telnyx_api_key,
)
files.put(
    name="Install b00t-server.service",
    src="files/b00t-server.service",
    dest="/etc/systemd/system/b00t-server.service",
)
systemd.service(
    name="Enable + start b00t-server",
    service="b00t-server.service",
    running=True,
    enabled=True,
    restarted=True,  # pick up a rebuilt binary / rotated key on re-run
    daemon_reload=True,
)


