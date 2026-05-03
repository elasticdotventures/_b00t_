#!/bin/bash
set -euo pipefail

# Build script for b00t-cli Docker container
# Usage: ./build-b00t-cli.sh [tag]

TAG=${1:-"b00t-cli:latest"}
VERSION=${2:-"0.0.1"}
COMMIT=$(git rev-parse HEAD)
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "🥾 Building b00t-cli Docker container..."
echo "Tag: $TAG"
echo "Version: $VERSION"
echo "Commit: $COMMIT"
echo "Build Date: $BUILD_DATE"

docker build \
    -f Dockerfile.b00t-cli \
    --build-arg BUILD_VERSION="$VERSION" \
    --build-arg BUILD_COMMIT="$COMMIT" \
    --build-arg BUILD_DATE="$BUILD_DATE" \
    -t "$TAG" \
    .

echo "✅ Build complete! Testing container..."
docker run --rm "$TAG" b00t-cli --version
echo "🎉 Container test successful!"