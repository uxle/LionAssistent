#!/usr/bin/env bash
# Renders assets/icons/ai-assistant.svg to the fixed PNG sizes Linux
# desktop icon themes and packaging formats actually expect
# (freedesktop.org Icon Theme Specification's standard sizes; AppImage
# and .deb both want at least a 256x256 for their own metadata too).
#
# Requires rsvg-convert (Debian/Ubuntu package: librsvg2-bin). Idempotent
# -- safe to re-run after editing the SVG.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

SVG="assets/icons/ai-assistant.svg"
SIZES=(16 32 48 64 128 256 512)

for size in "${SIZES[@]}"; do
    out="assets/icons/ai-assistant-${size}.png"
    rsvg-convert -w "$size" -h "$size" "$SVG" -o "$out"
    echo "wrote $out"
done
