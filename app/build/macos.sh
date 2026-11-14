#!/bin/sh
# Compile Taiseux pour macOS (application + dmg) sans laisser le dossier personnel dans le binaire :
# rustc y écrit sinon les chemins des sources (messages de panique), donc le nom du compte.
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
export RUSTFLAGS="--remap-path-prefix=$HOME=/b --remap-path-prefix=$here=/taiseux"
cd "$here/src-tauri"
cargo tauri build --bundles app,dmg
n=$(strings "$here/target/release/bundle/macos/Taiseux.app/Contents/MacOS/taiseux" | grep -c "$HOME" || true)
[ "$n" = 0 ] || { echo "!! le binaire contient encore $n fois le dossier personnel" >&2; exit 1; }
echo "binaire sans chemin personnel"
