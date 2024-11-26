#!/bin/bash
set -euo pipefail

echo "Needs to be logged in to docker hub to push images (docker login)"
read -p "Press Enter to continue or Ctrl+C to exit..."

# Get version from Cargo.toml
if ! version=$(grep -oP 'version = "\K[^"]+' Cargo.toml); then
    echo "Error: Failed to extract version from Cargo.toml"
    exit 1
fi

# Server tag
if ! git tag -a "$version" -m "Release $version"; then
    echo "Error: Failed to create server tag"
    exit 1
fi

if ! git push origin "$version"; then
    echo "Error: Failed to push server tag"
    exit 1
fi

# Client tag
cd frontend || exit 1
if ! git tag -a "v$version" -m "Release v$version"; then
    echo "Error: Failed to create client tag"
    exit 1
fi

if ! git push origin "v$version"; then
    echo "Error: Failed to push client tag"
    exit 1
fi
cd ..

# Docker build and push
if ! docker build -t "uratne/fefs:$version" .; then
    echo "Error: Docker build failed"
    exit 1
fi

if ! docker push "uratne/fefs:$version"; then
    echo "Error: Docker push failed"
    exit 1
fi

# Client binary build
if ! cargo build --target x86_64-unknown-linux-musl --release --bin client; then
    echo "Error: Failed to build client"
    exit 1
fi

mkdir -p ~/Documents/fefs
cp "target/x86_64-unknown-linux-musl/release/client" \
   "${HOME}/Documents/fefs/fefs_client_${version}_linux_x86_64"