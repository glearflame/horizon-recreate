#!/usr/bin/env bash
# play-offline.sh — inicia o L&L (Minecraft 26.3 em Rust) totalmente offline,
# sem conta Microsoft/Mojang. Nada de auth, nada de keychain.
set -euo pipefail

export JAVA_HOME="${JAVA_HOME:-$HOME/.java/temurin-25}"
export PATH="$JAVA_HOME/bin:$PATH"

RESOURCES="${RESOURCES:-$HOME/.horizon/resources}"
CLIENT="${CLIENT:-$HOME/.local/bin/ll-client}"

mkdir -p "$HOME/.ll-home"

echo "[ll] iniciando cliente offline (recursos em $RESOURCES)..."
exec "$CLIENT" \
  --resources "$RESOURCES" \
  --game-dir "$HOME/.ll-home" \
  --width 1280 --height 720 \
  --creative \
  "${@}"
