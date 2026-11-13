#!/bin/sh
# Construit dev/out/ (l'interface ui/ + l'imitation de Tauri) et le binaire dev_api, pour voir l'interface dans un
# navigateur avec le vrai moteur : ensuite `python3 dev/serveur.py` (port 8894). Développement seulement.
set -eu
d=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$d/out"
(cd "$d/.." && cargo build -q --release -p taiseux-core --example dev_api)
cp "$d/../ui/app.css" "$d/../ui/app.js" "$d/mock.js" "$d/out/"
sed 's#<script src="app.js"></script>#<script src="mock.js"></script><script src="app.js"></script>#' "$d/../ui/index.html" > "$d/out/index.html"
