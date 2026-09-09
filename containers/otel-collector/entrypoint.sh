#!/usr/bin/env sh
# otel-collector entrypoint — layer the downstream overlay only when a
# downstream endpoint is configured, so the base config is safe to run alone
# (no self-loop export). Plan gleaming-jingling-nygaard.
set -eu

set -- --config /etc/otelcol/otelcol-config.yaml
if [ -n "${OTEL_DOWNSTREAM_ENDPOINT:-}" ]; then
  set -- "$@" --config /etc/otelcol/otelcol-config.downstream.yaml
  echo "otelcol: downstream export -> ${OTEL_DOWNSTREAM_ENDPOINT}"
else
  echo "otelcol: local capture only (file + prometheus :8889); set OTEL_DOWNSTREAM_ENDPOINT to fan out"
fi

exec /otelcol-contrib "$@"
