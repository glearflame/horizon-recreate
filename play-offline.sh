#!/usr/bin/env bash
# play-offline.sh — inicia o L&L (Minecraft 26.3 em Rust) totalmente offline,
# sem conta Microsoft/Mojang. Nada de auth, nada de keychain.
set -euo pipefail

export JAVA_HOME="${JAVA_HOME:-$HOME/.java/temurin-25}"
export PATH="$JAVA_HOME/bin:$PATH"

RESOURCES="${RESOURCES:-$HOME/.horizon/resources}"
CLIENT="${CLIENT:-$HOME/.local/bin/ll-client}"

# Terminal hermetico pode herdar a sessao sem DISPLAY. Reconecta ao compositor
# grafico existente, sem abrir outro servidor nem tocar nos dados do jogador.
if [[ -z "${DISPLAY:-}" && -z "${WAYLAND_DISPLAY:-}" ]]; then
  runtime="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
  for socket in "$runtime"/wayland-*; do
    [[ -S "$socket" ]] || continue
    export XDG_RUNTIME_DIR="$runtime"
    export WAYLAND_DISPLAY="${socket##*/}"
    break
  done
  if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
    for socket in /tmp/.X11-unix/X*; do
      [[ -S "$socket" ]] || continue
      export DISPLAY=":${socket##*X}"
      break
    done
  fi
fi

mkdir -p "$HOME/.ll-home"

echo "[ll] iniciando cliente offline (recursos em $RESOURCES)..."
exec "$CLIENT" \
  --resources "$RESOURCES" \
  --game-dir "$HOME/.ll-home" \
  --width 1280 --height 720 \
  --creative \
  "${@}"
