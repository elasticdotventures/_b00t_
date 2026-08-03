set export
set shell := ["bash", "-c"]

COMPONENT_DIR := parent_directory(source_file())
DEMO_ROOT := env_var_or_default("PINGAP_DEMO_ROOT", "/tmp/pingap-devproxy-demo")
DEMO_HOST := env_var_or_default("PINGAP_DEMO_HOST", "demo.pingap.test")
BACKEND_PORT := env_var_or_default("PINGAP_DEMO_BACKEND_PORT", "19091")
HTTPS_PORT := env_var_or_default("PINGAP_DEMO_HTTPS_PORT", "19443")
HTTP_PORT := env_var_or_default("PINGAP_DEMO_HTTP_PORT", "19080")
POD_NAME := env_var_or_default("PINGAP_DEMO_POD_NAME", "pingap-devproxy-demo")
IMAGE := env_var_or_default("PINGAP_DEMO_IMAGE", "localhost/pingap-devproxy-b00t:demo")

default: up

up: write-files build generate start-backend deploy check

write-files:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "$DEMO_ROOT/_b00t_/k8s/pingap" "$DEMO_ROOT/certs" "$DEMO_ROOT/site"

    cat > "$DEMO_ROOT/site/index.html" <<'HTML'
    <!doctype html>
    <html lang="en">
      <head>
        <meta charset="utf-8">
        <title>Pingap devproxy b00t demo</title>
      </head>
      <body>
        <h1>pingap-devproxy-b00t functional demo</h1>
        <p>Backend reached through Pingap.</p>
      </body>
    </html>
    HTML

    cat > "$DEMO_ROOT/_b00t_/k8s/pingap/services.mjs" <<EOF_SERVICES
    export const services = [
      {
        name: "demo",
        host: "$DEMO_HOST",
        upstreamName: "demoUpstream",
        target: "host.containers.internal:$BACKEND_PORT",
        certGroup: "demo",
        healthPath: "/",
        healthStatuses: [200],
      },
    ];

    export const certificates = {
      demo: {
        tls_cert: "/certs/$DEMO_HOST.pem",
        tls_key: "/certs/$DEMO_HOST-key.pem",
        domains: "$DEMO_HOST",
        is_default: true,
      },
    };
    EOF_SERVICES

    if [ ! -f "$DEMO_ROOT/certs/$DEMO_HOST.pem" ] || [ ! -f "$DEMO_ROOT/certs/$DEMO_HOST-key.pem" ]; then
        openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
            -subj "/CN=$DEMO_HOST" \
            -keyout "$DEMO_ROOT/certs/$DEMO_HOST-key.pem" \
            -out "$DEMO_ROOT/certs/$DEMO_HOST.pem" >/dev/null 2>&1
    fi

build:
    @podman build -f "$COMPONENT_DIR/Containerfile" -t "$IMAGE" "$COMPONENT_DIR"

generate:
    @PINGAP_PROJECT_ROOT="$DEMO_ROOT" "$COMPONENT_DIR/bin/gen-pingap-config.mjs"

start-backend:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "$DEMO_ROOT"
    if pgrep -f "http.server $BACKEND_PORT --bind 0.0.0.0" > "$DEMO_ROOT/backend.pid"; then
        exit 0
    fi
    setsid -f bash -c "cd '$DEMO_ROOT/site' && exec python3 -m http.server '$BACKEND_PORT' --bind 0.0.0.0 > '$DEMO_ROOT/backend.log' 2>&1"
    sleep 1
    pgrep -n -f "http.server $BACKEND_PORT --bind 0.0.0.0" > "$DEMO_ROOT/backend.pid"

deploy:
    @env PINGAP_PROJECT_ROOT="$DEMO_ROOT" \
        PINGAP_POD_NAME="$POD_NAME" \
        PINGAP_IMAGE="$IMAGE" \
        PINGAP_SHADOW_HTTPS_PORT="$HTTPS_PORT" \
        PINGAP_SHADOW_HTTP_PORT="$HTTP_PORT" \
        "$COMPONENT_DIR/bin/pingap-kube-play.sh" --shadow

check:
    @env PINGAP_PROJECT_ROOT="$DEMO_ROOT" \
        PINGAP_POD_NAME="$POD_NAME" \
        PINGAP_IMAGE="$IMAGE" \
        PINGAP_SHADOW_HTTPS_PORT="$HTTPS_PORT" \
        PINGAP_SHADOW_HTTP_PORT="$HTTP_PORT" \
        "$COMPONENT_DIR/bin/pingap-kube-play.sh" --status --json

status:
    #!/usr/bin/env bash
    set -euo pipefail
    if env PINGAP_PROJECT_ROOT="$DEMO_ROOT" \
        PINGAP_POD_NAME="$POD_NAME" \
        PINGAP_IMAGE="$IMAGE" \
        PINGAP_SHADOW_HTTPS_PORT="$HTTPS_PORT" \
        PINGAP_SHADOW_HTTP_PORT="$HTTP_PORT" \
        "$COMPONENT_DIR/bin/pingap-kube-play.sh" --status --json; then
        exit 0
    fi

    cat >&2 <<'EOF'

    Demo status is unhealthy or not initialized.

    Run:
      just demo::up

    For teardown:
      just demo::down
    EOF

down:
    #!/usr/bin/env bash
    set -euo pipefail
    env PINGAP_PROJECT_ROOT="$DEMO_ROOT" \
        PINGAP_POD_NAME="$POD_NAME" \
        "$COMPONENT_DIR/bin/pingap-kube-play.sh" --down || true
    if [ -f "$DEMO_ROOT/backend.pid" ]; then
        kill "$(cat "$DEMO_ROOT/backend.pid")" >/dev/null 2>&1 || true
        rm -f "$DEMO_ROOT/backend.pid"
    fi
