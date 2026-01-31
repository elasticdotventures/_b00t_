#!/usr/bin/env bash
set -euo pipefail

: "${AWS_REGION:=us-east-1}"            # region for CLI commands
: "${AWS_S3_BUCKET:?set AWS_S3_BUCKET}"  # target bucket name (required)
: "${AWS_S3_PREFIX:=}"                   # optional prefix filter
: "${AWS_S3_DOWNLOAD_KEY:=}"             # optional key to download
: "${AWS_S3_DOWNLOAD_DEST:=/tmp}"        # destination directory for downloads

LISTING="/tmp/aws_s3_listing.txt"
rm -f "$LISTING"

printf '🔎 Listing s3://%s/%s\n' "$AWS_S3_BUCKET" "$AWS_S3_PREFIX"
aws s3api list-objects-v2 \
  --bucket "$AWS_S3_BUCKET" \
  ${AWS_S3_PREFIX:+--prefix "$AWS_S3_PREFIX"} \
  --region "$AWS_REGION" \
  --output json | jq -r '.Contents[] | [.Key,.Size,.LastModified] | @tsv' | tee "$LISTING"
# output: TSV of Key Size Timestamp written to $LISTING

if [ -n "$AWS_S3_DOWNLOAD_KEY" ]; then
  DEST_PATH="$AWS_S3_DOWNLOAD_DEST/$(basename "$AWS_S3_DOWNLOAD_KEY")"
  printf '⬇️  Downloading s3://%s/%s -> %s\n' "$AWS_S3_BUCKET" "$AWS_S3_DOWNLOAD_KEY" "$DEST_PATH"
  mkdir -p "$AWS_S3_DOWNLOAD_DEST"
  aws s3 cp \
    "s3://$AWS_S3_BUCKET/$AWS_S3_DOWNLOAD_KEY" \
    "$DEST_PATH" \
    --region "$AWS_REGION"
  printf '✅ Downloaded to %s\n' "$DEST_PATH"
fi
