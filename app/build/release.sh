#!/usr/bin/env bash
# Rassemble les paquets de Taiseux dans dist/<version>/, calcule SHA256SUMS et horodate (OpenTimestamps).
#
#   build/release.sh            (après : tests, build/macos.sh sur le Mac, build/linux.sh, build/windows.sh)
#
# Ne prend que ce qui existe ; dit ce qui manque. Les archives (.zip / .tar.gz) sont ce que ToolGuild livre par curl.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
V=$(grep -m1 '^version' "$here/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')
OUT="$here/dist/$V"
mkdir -p "$OUT"
cd "$here"

echo "== Tests du moteur et du coffre"
cargo test -p taiseux-core --quiet 2>&1 | grep -E "test result" | sed 's/^/   /'

manque=()
# macOS Apple Silicon : l'application en zip (ditto garde les attributs du paquet).
app="target/release/bundle/macos/Taiseux.app"
if [ -d "$app" ]; then
  rm -f "$OUT/taiseux-$V-macos-arm64.zip"
  ditto -c -k --keepParent "$app" "$OUT/taiseux-$V-macos-arm64.zip"
  cp target/release/bundle/dmg/Taiseux_"$V"_aarch64.dmg "$OUT/taiseux-$V-macos-arm64.dmg" 2>/dev/null || true
  # LISEZMOI et LICENSE (MIT) ajoutés au zip (zip -j : pas de fichier parasite « ._ » de ditto).
  zip -qj "$OUT/taiseux-$V-macos-arm64.zip" build/LISEZMOI.txt ../LICENSE
else manque+=("macos-arm64"); fi

# Linux : AppImage (autonome) et .deb, dans une archive par architecture.
for arch in arm64 amd64; do
  d="dist/linux-$arch"
  if ls "$d"/*.AppImage >/dev/null 2>&1; then
    tmp=$(mktemp -d); n="taiseux-$V-linux-$arch"; mkdir "$tmp/$n"
    cp "$d"/*.AppImage "$tmp/$n/Taiseux.AppImage"; cp "$d"/*.deb "$tmp/$n/"; chmod +x "$tmp/$n/Taiseux.AppImage"
    cp build/LISEZMOI.txt ../LICENSE "$tmp/$n/"
    # Sans le nom du compte qui compile (propriétaire des fichiers) ni fichiers « ._ » de macOS.
    COPYFILE_DISABLE=1 tar -C "$tmp" --uid 0 --gid 0 --uname root --gname root -czf "$OUT/$n.tar.gz" "$n"; rm -rf "$tmp"
  else manque+=("linux-$arch"); fi
done

# Windows x64 : exécutable portable + installeur.
if [ -f dist/windows-x64/taiseux.exe ]; then
  tmp=$(mktemp -d); n="taiseux-$V-windows-x64"; mkdir "$tmp/$n"
  cp dist/windows-x64/taiseux.exe dist/windows-x64/WebView2Loader.dll build/LISEZMOI.txt ../LICENSE "$tmp/$n/"
  (cd "$tmp" && zip -qr "$OUT/$n.zip" "$n"); rm -rf "$tmp"
else manque+=("windows-x64"); fi

( cd "$OUT" && shopt -s nullglob && f=(*.zip *.tar.gz *.dmg *.exe) && shasum -a 256 "${f[@]}" > SHA256SUMS && cat SHA256SUMS | sed 's/^/   /' )

echo "== Horodatage OpenTimestamps"
"$here/../../outils/ots-release.sh" taiseux "$V" "$OUT" taiseux || true

[ ${#manque[@]} -eq 0 ] || echo "!! Paquets absents : ${manque[*]}"
echo "== $OUT"
