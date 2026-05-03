#!/bin/bash

cargo bump patch

# Extract the version from Cargo.toml
CARGO_VERSION=$(cargo pkgid | cut -d# -f2 | cut -d: -f2)
echo "Extracted Cargo Version: $CARGO_VERSION"

# Check if CARGO_VERSION is blank
if [ -z "$CARGO_VERSION" ]; then
  echo "Cargo version is empty, exiting..."
  exit 1
fi

# Update pyproject.toml
OLD_VERSION=$(grep '^version' pyproject.toml | head -1 | cut -d '"' -f 2)
sed -i "s+version = \"$OLD_VERSION\"+version = \"$CARGO_VERSION\"+g" pyproject.toml

# Update package.json
jq ".version = \"$CARGO_VERSION\"" package.json > package.tmp.json && mv package.tmp.json package.json

# Git commit
git add Cargo.toml pyproject.toml package.json
git commit -m "Update version to $CARGO_VERSION"

# Create and push tag
TAG_VERSION="v$CARGO_VERSION"
git tag $TAG_VERSION
git push origin $TAG_VERSION

echo "Version updated to $CARGO_VERSION and tagged as $TAG_VERSION"
