#!/bin/sh
# Compile Taiseux pour Linux dans Docker : build/linux.sh amd64|arm64 → dist/linux-<arch>/
set -eu
arch=${1:-arm64}
here=$(cd "$(dirname "$0")/.." && pwd)
docker build --platform "linux/$arch" -t "taiseux-build:$arch" -f "$here/build/Dockerfile.linux" "$here/build"
mkdir -p "$here/dist/linux-$arch"
# target/ propre à l'architecture, hors du target/ macOS ; les sources sont montées en lecture seule.
docker run --rm --platform "linux/$arch" \
  -v "$here:/src:ro" -v "taiseux-target-$arch:/target" -v "$here/dist/linux-$arch:/out" \
  -e CARGO_TARGET_DIR=/target -e APPIMAGE_EXTRACT_AND_RUN=1 \
  "taiseux-build:$arch" sh -c 'mkdir -p /w && tar -C /src --exclude=./target --exclude=./dist -cf - . | tar -C /w -xf - && cd /w/src-tauri && cargo tauri build --bundles deb,appimage && cp /target/release/bundle/deb/*.deb /target/release/bundle/appimage/*.AppImage /out/'
ls -l "$here/dist/linux-$arch"
