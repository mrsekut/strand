#!/usr/bin/env bash
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Usage: ./scripts/release.sh <version>"
  echo "Example: ./scripts/release.sh 0.2.0"
  exit 1
fi

VERSION="$1"

echo "==> Updating version to $VERSION"

# Update Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Update flake.nix
sed -i '' "s/version = \".*\";/version = \"$VERSION\";/" flake.nix

# Build check
echo "==> Building..."
cargo build

echo "==> Committing..."
git add Cargo.toml Cargo.lock flake.nix
git commit -m "chore: bump version to v$VERSION"
git push

echo "==> Publishing to crates.io..."
cargo publish

echo "==> Tagging v$VERSION..."
git tag "v$VERSION"
git push origin "v$VERSION"

echo "==> Done! v$VERSION released."
