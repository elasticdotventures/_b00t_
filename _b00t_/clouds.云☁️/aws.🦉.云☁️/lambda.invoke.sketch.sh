#!/usr/bin/env bash
set -euo pipefail

# Minimal CLI-first Lambda deploy+invoke sketch.
: "${AWS_REGION:=us-east-1}"          # default region for CLI commands
: "${AWS_LAMBDA_FUNCTION:=sample-b00t-lambda}"  # target Lambda name
: "${LAMBDA_SRC:=lambda-src}"          # directory containing handler + deps
: "${LAMBDA_HANDLER:=app.handler}"     # python style handler entrypoint
: "${LAMBDA_RUNTIME:=python3.12}"      # runtime identifier
: "${LAMBDA_TIMEOUT:=30}"              # timeout seconds for the invoke smoke

PAYLOAD_ZIP="/tmp/lambda_payload.zip"
DEPLOY_RECEIPT="/tmp/lambda_deploy.json"
INVOKE_PAYLOAD="/tmp/lambda_invoke_payload.json"

if [ ! -d "$LAMBDA_SRC" ]; then
  echo "⚠️  create $LAMBDA_SRC with a handler before running" >&2
  exit 2
fi

rm -f "$PAYLOAD_ZIP"
(cd "$LAMBDA_SRC" && zip -qr "$PAYLOAD_ZIP" .)

aws lambda update-function-configuration \
  --function-name "$AWS_LAMBDA_FUNCTION" \
  --handler "$LAMBDA_HANDLER" \
  --runtime "$LAMBDA_RUNTIME" \
  --timeout "$LAMBDA_TIMEOUT" \
  --region "$AWS_REGION" \
  > "$DEPLOY_RECEIPT"
# output: JSON confirmation of configuration update

aws lambda update-function-code \
  --function-name "$AWS_LAMBDA_FUNCTION" \
  --zip-file "fileb://$PAYLOAD_ZIP" \
  --publish \
  --region "$AWS_REGION" \
  >> "$DEPLOY_RECEIPT"
# output: JSON block containing new Version + CodeSha256

aws lambda invoke \
  --function-name "$AWS_LAMBDA_FUNCTION" \
  --payload '{"ping":"cake"}' \
  --cli-binary-format raw-in-base64-out \
  --log-type Tail \
  --region "$AWS_REGION" \
  "$INVOKE_PAYLOAD"
# output: writes response body to $INVOKE_PAYLOAD and emits requestId + duration

jq '.' "$DEPLOY_RECEIPT"
cat "$INVOKE_PAYLOAD"
