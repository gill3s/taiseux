#!/bin/sh
# Compile Taiseux pour Windows x64 dans Docker → dist/windows-x64/taiseux.exe (portable, sans installeur : le NSIS de
# Debian 3.08 n'a pas Win\\RestartManager.nsh qu'exige Tauri ; WebView2 est déjà présent sous Windows 10 et 11).
# Jamais lancé sur une vraie machine Windows tant que ce n'est pas écrit dans le journal.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
docker build -t taiseux-build:windows -f "$here/build/Dockerfile.windows" "$here/build"
mkdir -p "$here/dist/windows-x64"
docker run --rm -v "$here:/src:ro" -v "taiseux-target-windows:/target" -v "$here/dist/windows-x64:/out" \
  -e CARGO_TARGET_DIR=/target "taiseux-build:windows" sh -c '
    mkdir -p /w && tar -C /src --exclude=./target --exclude=./dist -cf - . | tar -C /w -xf - && cd /w/src-tauri &&
    cargo tauri build --target x86_64-pc-windows-gnu --no-bundle &&
    cp /target/x86_64-pc-windows-gnu/release/taiseux.exe /target/x86_64-pc-windows-gnu/release/WebView2Loader.dll /out/'
ls -l "$here/dist/windows-x64"
