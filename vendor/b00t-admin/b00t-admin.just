# b00t-admin recipes — build, run, and manage the admin server container
# Module: vendor/b00t-admin/b00t-admin.just
# Namespaced as: just b00t-admin <recipe>

# ── Container Build ────────────────────────────────────────────────────────

# Build the b00t-admin container image
build:
    #!/bin/bash
    set -euo pipefail
    echo "🔨 Building b00t-admin container..."
    podman build \
        --tag b00t-admin:latest \
        --file b00t-admin/Containerfile \
        .
    echo "✅ b00t-admin:latest built"

# Build and tag for registry push
build-release version:
    #!/bin/bash
    set -euo pipefail
    TAG="{{version}}"
    echo "🔨 Building b00t-admin:${TAG}..."
    podman build \
        --tag b00t-admin:${TAG} \
        --tag ghcr.io/elasticdotventures/b00t-admin:${TAG} \
        --file b00t-admin/Containerfile \
        .
    echo "✅ b00t-admin:${TAG} built"

# Push to GitHub Container Registry
push-release version:
    podman push ghcr.io/elasticdotventures/b00t-admin:{{version}}

# ── Run ────────────────────────────────────────────────────────────────────

# Run b00t-admin container (detached)
run:
    #!/bin/bash
    set -euo pipefail
    echo "🚀 Starting b00t-admin on :31337..."
    podman run -d --replace --name b00t-admin \
        -p 31337:31337 \
        -e LLM_BACKEND_URL=http://host.containers.internal:5273 \
        -e ADMIN_PORT=31337 \
        -v "${HOME}/.b00t/admin:/home/b00t/.b00t/admin:Z" \
        b00t-admin:latest
    echo "✅ b00t-admin running"
    echo "   Dashboard: http://localhost:31337/"
    echo "   Logs:      podman logs b00t-admin"

# Run in foreground (for debugging)
run-debug:
    podman run --rm -it \
        -p 31337:31337 \
        -e LLM_BACKEND_URL=http://host.containers.internal:5273 \
        -e ADMIN_PORT=31337 \
        b00t-admin:latest

# Stop and remove the container
stop:
    podman stop b00t-admin 2>/dev/null || true
    podman rm b00t-admin 2>/dev/null || true

# View logs
logs:
    podman logs -f b00t-admin

# ── Quadlet (systemd) ──────────────────────────────────────────────────────

# Install quadlet for systemd auto-restart
quadlet-install:
    #!/bin/bash
    set -euo pipefail
    mkdir -p "${HOME}/.config/containers/systemd"
    cp b00t-admin/quadlet/b00t-admin.container "${HOME}/.config/containers/systemd/"
    systemctl --user daemon-reload
    echo "✅ Quadlet installed"
    echo "   Start:  systemctl --user start b00t-admin"
    echo "   Status: systemctl --user status b00t-admin"
    echo "   Logs:   journalctl --user -u b00t-admin -f"

# Remove quadlet
quadlet-remove:
    rm -f "${HOME}/.config/containers/systemd/b00t-admin.container"
    systemctl --user daemon-reload
    systemctl --user stop b00t-admin 2>/dev/null || true

# ── Kube Play (alternative to quadlet) ─────────────────────────────────────

# Deploy via podman kube play
kube-deploy:
    podman kube play b00t-admin/kube/b00t-admin.yaml

# Tear down kube deployment
kube-teardown:
    podman kube down b00t-admin/kube/b00t-admin.yaml

# ── Binary (no container) ──────────────────────────────────────────────────

# Build and run directly (no container)
run-native:
    #!/bin/bash
    set -euo pipefail
    cargo build --release -p b00t-admin
    echo "🚀 Starting b00t-admin natively on :31337..."
    LLM_BACKEND_URL=http://localhost:5273 ADMIN_PORT=31337 ./target/release/b00t-admin

# ── Status ─────────────────────────────────────────────────────────────────

# Check if b00t-admin is healthy
status:
    #!/bin/bash
    curl -s http://localhost:31337/api/admin/pipeline 2>/dev/null | python3 -m json.tool 2>/dev/null || echo "❌ b00t-admin not reachable on :31337"
