#!/usr/bin/env bash
set -euo pipefail

# Generic Pingap kube-play lifecycle wrapper. Project-specific route/cert data
# stays in the consuming repo; this script only renders and deploys it.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPONENT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PROJECT_ROOT="${PINGAP_PROJECT_ROOT:-$PWD}"
K8S_DIR="${PINGAP_K8S_DIR:-$PROJECT_ROOT/_b00t_/k8s}"
CONFIG_SOURCE_DIR="${PINGAP_CONFIG_SOURCE_DIR:-$K8S_DIR/pingap}"
GENERATED_DIR="${PINGAP_GENERATED_DIR:-$K8S_DIR/.generated}"
POD_TEMPLATE="${PINGAP_POD_TEMPLATE:-$COMPONENT_DIR/templates/pingap.pod.yml.tmpl}"
POD_NAME="${PINGAP_POD_NAME:-pingap-devproxy-b00t}"
POD_FILE="${PINGAP_POD_FILE:-$GENERATED_DIR/${POD_NAME}.pod.yml}"
STATE_FILE="${PINGAP_STATE_FILE:-$GENERATED_DIR/${POD_NAME}.mode}"
CERTS_DIR="${PINGAP_CERTS_DIR:-$PROJECT_ROOT/certs}"
PINGAP_IMAGE="${PINGAP_IMAGE:-docker.io/vicanso/pingap:latest}"

set_mode_ports() {
    case "$1" in
        shadow)
            PINGAP_HTTPS_PORT="${PINGAP_SHADOW_HTTPS_PORT:-18443}"
            PINGAP_HTTP_PORT="${PINGAP_SHADOW_HTTP_PORT:-18080}"
            ;;
        cutover)
            PINGAP_HTTPS_PORT="${PINGAP_CUTOVER_HTTPS_PORT:-8443}"
            PINGAP_HTTP_PORT="${PINGAP_CUTOVER_HTTP_PORT:-8080}"
            ;;
        *) echo "Unknown mode: $1 (expected shadow|cutover)" >&2; exit 1 ;;
    esac
}

load_checks() {
    if [ -n "${PINGAP_STATUS_HOSTS:-}" ]; then
        while IFS= read -r host; do
            [ -n "$host" ] || continue
            printf '%s\t/\t200\n' "$host"
        done < <(tr ',' '\n' <<< "$PINGAP_STATUS_HOSTS")
        return
    fi
    PINGAP_SERVICES_MODULE="${PINGAP_SERVICES_MODULE:-$CONFIG_SOURCE_DIR/services.mjs}" \
        node --input-type=module -e '
            const mod = await import("file://" + process.env.PINGAP_SERVICES_MODULE);
            for (const service of mod.services ?? []) {
                const path = service.healthPath ?? "/";
                const statuses =
                    service.healthStatuses ??
                    service.expectedStatuses ??
                    [service.healthStatus ?? service.expectedStatus ?? 200];
                console.log([service.host, path, statuses.join(",")].join("\t"));
            }
        '
}

render() {
    local mode="$1"
    set_mode_ports "$mode"
    export PROJECT_ROOT K8S_DIR CONFIG_SOURCE_DIR GENERATED_DIR POD_TEMPLATE
    export PINGAP_POD_NAME="$POD_NAME"
    export PINGAP_IMAGE
    export CERTS_DIR
    export PINGAP_HTTPS_PORT PINGAP_HTTP_PORT
    export PINGAP_LISTEN_ADDR="0.0.0.0:${PINGAP_HTTPS_PORT}"
    export PINGAP_HTTP_ADDR="0.0.0.0:${PINGAP_HTTP_PORT}"
    export GENERATED_CONFIG_DIR="$GENERATED_DIR/pingap-config"

    local vars='${PINGAP_POD_NAME} ${PINGAP_IMAGE} ${CERTS_DIR} ${PINGAP_HTTPS_PORT} ${PINGAP_HTTP_PORT} ${PINGAP_LISTEN_ADDR} ${PINGAP_HTTP_ADDR} ${GENERATED_CONFIG_DIR}'

    mkdir -p "$GENERATED_CONFIG_DIR"
    for f in "$CONFIG_SOURCE_DIR"/*.toml; do
        envsubst "$vars" < "$f" > "$GENERATED_CONFIG_DIR/$(basename "$f")"
    done
    envsubst "$vars" < "$POD_TEMPLATE" > "$POD_FILE"
    echo "$mode" > "$STATE_FILE"
}

deploy() {
    local mode="${1:-shadow}"
    render "$mode"
    echo "Deploying $POD_NAME ($mode: ${PINGAP_HTTPS_PORT}/${PINGAP_HTTP_PORT})"
    podman kube down "$POD_FILE" >/dev/null 2>&1 || true
    podman kube play "$POD_FILE" 2>&1 | sed 's/^/  /'
    echo "deployed: $POD_NAME"
}

teardown() {
    echo "Tearing down $POD_NAME"
    if [ -f "$POD_FILE" ]; then
        podman kube down "$POD_FILE" 2>&1 | sed 's/^/  /' || true
    else
        podman pod rm -f "$POD_NAME" 2>&1 | sed 's/^/  /' || true
    fi
}

current_mode() {
    [ -f "$STATE_FILE" ] && cat "$STATE_FILE" || echo "shadow"
}

check_host() {
    local host="$1" port="$2" path="${3:-/}"
    local code
    code="$(curl -sk -o /dev/null -w "%{http_code}" --resolve "$host:$port:127.0.0.1" "https://$host:$port$path" -m 5 2>/dev/null || true)"
    printf '%s' "${code:-000}"
}

check_redirect() {
    local host="$1" port="$2"
    local code
    code="$(curl -s -o /dev/null -w "%{http_code}" --resolve "$host:$port:127.0.0.1" "http://$host:$port/" -m 5 2>/dev/null || true)"
    printf '%s' "${code:-000}"
}

matches_expected() {
    local code="$1" expected_csv="$2"
    local expected
    IFS=',' read -ra expected <<< "$expected_csv"
    for status in "${expected[@]}"; do
        [ "$code" = "$status" ] && return 0
    done
    return 1
}

status_text() {
    local mode; mode="$(current_mode)"
    set_mode_ports "$mode"
    echo "=== $POD_NAME Pod (mode: $mode) ==="
    podman pod ps --filter name="$POD_NAME" --format "table {{.Name}} {{.Status}} {{.Created}}"
    echo ""
    echo "=== Container Logs (last 10) ==="
    podman logs --tail 10 "$POD_NAME-pingap" 2>&1 | sed 's/^/  /' || true
    echo ""
    echo "=== Test routes ==="
    local any_fail=0
    local first_host=""
    while IFS=$'\t' read -r host path expected; do
        [ -n "$host" ] || continue
        [ -n "$first_host" ] || first_host="$host"
        code=$(check_host "$host" "$PINGAP_HTTPS_PORT" "$path")
        echo "  https://$host:$PINGAP_HTTPS_PORT$path -> $code (expected: $expected)"
        matches_expected "$code" "$expected" || any_fail=1
    done < <(load_checks)
    if [ -n "$first_host" ]; then
        code=$(check_redirect "$first_host" "$PINGAP_HTTP_PORT")
        echo "  http://$first_host:$PINGAP_HTTP_PORT/ -> $code (expected: 301)"
        [ "$code" = "301" ] || any_fail=1
    fi
    return $any_fail
}

status_json() {
    local mode; mode="$(current_mode)"
    set_mode_ports "$mode"
    local pod_status
    pod_status="$(podman pod ps --filter name="$POD_NAME" --format "{{.Status}}" 2>/dev/null || echo "unknown")"
    local any_fail=0
    local rows=()
    local first_host=""
    local redirect_code="000"
    while IFS=$'\t' read -r host path expected; do
        [ -n "$host" ] || continue
        [ -n "$first_host" ] || first_host="$host"
        code=$(check_host "$host" "$PINGAP_HTTPS_PORT" "$path")
        matches_expected "$code" "$expected" || any_fail=1
        rows+=("$host" "$path" "$expected" "$code")
    done < <(load_checks)
    if [ -n "$first_host" ]; then
        redirect_code=$(check_redirect "$first_host" "$PINGAP_HTTP_PORT")
        [ "$redirect_code" = "301" ] || any_fail=1
    fi
    MODE="$mode" POD_NAME="$POD_NAME" POD_STATUS="$pod_status" HTTPS_PORT="$PINGAP_HTTPS_PORT" HTTP_PORT="$PINGAP_HTTP_PORT" REDIRECT_HOST="$first_host" REDIRECT_CODE="$redirect_code" \
        python3 -c "
import json, os, sys
rows = sys.argv[1:]
checks = [
    {
        'host': rows[i],
        'path': rows[i+1],
        'port': int(os.environ['HTTPS_PORT']),
        'expected_statuses': [int(s) for s in rows[i+2].split(',') if s],
        'http_code': rows[i+3],
        'reachable': rows[i+3] in rows[i+2].split(','),
    }
    for i in range(0, len(rows), 4)
]
print(json.dumps({
    'pod_name': os.environ['POD_NAME'],
    'mode': os.environ['MODE'],
    'pod_status': os.environ['POD_STATUS'],
    'https_port': int(os.environ['HTTPS_PORT']),
    'http_port': int(os.environ['HTTP_PORT']),
    'checks': checks,
    'redirect': {
        'host': os.environ['REDIRECT_HOST'],
        'port': int(os.environ['HTTP_PORT']),
        'expected_status': 301,
        'http_code': os.environ['REDIRECT_CODE'],
        'reachable': os.environ['REDIRECT_CODE'] == '301',
    },
}, indent=2))
" "${rows[@]}"
    return $any_fail
}

cutover_dryrun() {
    echo "Cutover dry-run for $POD_NAME: stop legacy proxy, deploy cutover, test, restore."
    nginx_was_running=0
    if pm2 describe nginx-proxy >/dev/null 2>&1 && pm2 describe nginx-proxy | grep -q "status.*online"; then
        nginx_was_running=1
    fi

    restore() {
        echo ""
        echo "Restoring legacy proxy state"
        teardown >/dev/null 2>&1 || true
        if [ "$nginx_was_running" = "1" ]; then
            pm2 restart nginx-proxy >/dev/null 2>&1 || pm2 start nginx-proxy >/dev/null 2>&1 || true
        fi
    }
    trap restore EXIT

    if [ "$nginx_was_running" = "1" ]; then
        pm2 stop nginx-proxy >/dev/null 2>&1 || true
    fi

    deploy cutover
    sleep 2
    status_text
}

case "${1:-}" in
    --down) teardown ;;
    --status)
        if [ "${2:-}" = "--json" ]; then status_json; else status_text || true; fi
        ;;
    --cutover-dryrun) cutover_dryrun ;;
    --cutover) deploy cutover ;;
    --shadow|"") deploy shadow ;;
    *)
        echo "Usage: $0 [--shadow|--cutover|--down|--status [--json]|--cutover-dryrun]" >&2
        exit 1
        ;;
esac
