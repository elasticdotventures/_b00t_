#!/usr/bin/env bash
# Run one compile-free nextest partition on k0s (vultr1) with the SPIRE
# workload identity — the keyless-GCS path dstack's kubernetes backend can't
# carry (b00t task #193). Apply, wait, stream logs, report the Job's result.
#
#   deploy/k0s-ci-test/run.sh <CI_ARCHIVE_KEY> [NEXTEST_PARTITION]
#
#   CI_ARCHIVE_KEY     ci-archives/<run>-<attempt>  (published by ci-build.task.yaml)
#   NEXTEST_PARTITION  default 1/16  (vultr1 is small)
#
# kubectl is reached via the tailnet: `ssh root@100.109.101.1 'k0s kubectl …'`.
# Override with KUBECTL="k0s kubectl" when run on the node itself.
set -euo pipefail

CI_ARCHIVE_KEY="${1:?usage: run.sh <CI_ARCHIVE_KEY> [NEXTEST_PARTITION]}"
NEXTEST_PARTITION="${2:-1/16}"
JOB_SUFFIX="$(date -u +%Y%m%d%H%M%S)-$RANDOM"
KUBECTL="${KUBECTL:-ssh -o ConnectTimeout=12 root@100.109.101.1 k0s kubectl}"
here="$(cd "$(dirname "$0")" && pwd)"

echo "archive=$CI_ARCHIVE_KEY partition=$NEXTEST_PARTITION job=b00t-ci-test-$JOB_SUFFIX"

$KUBECTL apply -f - < "$here/configmap.yaml"

export CI_ARCHIVE_KEY NEXTEST_PARTITION JOB_SUFFIX
envsubst '${CI_ARCHIVE_KEY} ${NEXTEST_PARTITION} ${JOB_SUFFIX}' < "$here/job.yaml" | $KUBECTL apply -f -

job="job/b00t-ci-test-$JOB_SUFFIX"
echo "--- waiting (Complete or Failed) ---"
$KUBECTL -n b00t-ci wait "$job" --for=condition=Complete --timeout=2400s &
w1=$!
$KUBECTL -n b00t-ci wait "$job" --for=condition=Failed --timeout=2400s &
w2=$!
wait -n "$w1" "$w2" || true
kill "$w1" "$w2" 2>/dev/null || true

$KUBECTL -n b00t-ci logs "$job" --all-containers --tail=-1 || true
echo "--- status ---"
$KUBECTL -n b00t-ci get "$job" -o jsonpath='{.status.conditions[*].type}{"\n"}'
