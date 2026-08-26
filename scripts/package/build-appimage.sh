#!/usr/bin/env bash
# Builds an AppImage for ai-assistant (Development_plan Phase 20
# "Packaging" -- AppImage).
#
# Usage: scripts/package/build-appimage.sh [path-to-release-binary]
#
# Needs appimagetool on PATH or at $APPIMAGETOOL -- download from
# https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BINARY="${1:-target/release/ai-assistant}"
APPIMAGETOOL="${APPIMAGETOOL:-appimagetool}"
VERSION=$(grep -A2 '^\[workspace.package\]' Cargo.toml | grep '^version' | head -1 | sed -E 's/version = "(.*)"/\1/')
OUT_DIR="target/package"
APPDIR="$OUT_DIR/AppDir"

if [[ ! -f "$BINARY" ]]; then
    echo "error: binary not found at $BINARY -- build it first (cargo build --release -p app)" >&2
    exit 1
fi
if ! command -v "$APPIMAGETOOL" >/dev/null 2>&1 && [[ ! -x "$APPIMAGETOOL" ]]; then
    echo "error: appimagetool not found (set \$APPIMAGETOOL or put it on PATH)" >&2
    exit 1
fi

echo "Building ai-assistant-${VERSION}-x86_64.AppImage from $BINARY"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin"
mkdir -p "$APPDIR/usr/share/applications"
for size in 16 32 48 64 128 256 512; do
    mkdir -p "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps"
done

install -m 755 "$BINARY" "$APPDIR/usr/bin/ai-assistant"
install -m 755 packaging/appimage/AppRun "$APPDIR/AppRun"
install -m 644 packaging/shared/ai-assistant.desktop "$APPDIR/ai-assistant.desktop"
install -m 644 packaging/shared/ai-assistant.desktop "$APPDIR/usr/share/applications/ai-assistant.desktop"
mkdir -p "$APPDIR/usr/share/metainfo"
install -m 644 packaging/shared/com.lionos.AiAssistant.metainfo.xml \
    "$APPDIR/usr/share/metainfo/com.lionos.AiAssistant.metainfo.xml"
# AppImage convention: the top-level icon can be the highest-res PNG (or
# an SVG) named to match the desktop file's Icon= key.
install -m 644 assets/icons/ai-assistant-256.png "$APPDIR/ai-assistant.png"
for size in 16 32 48 64 128 256 512; do
    install -m 644 "assets/icons/ai-assistant-${size}.png" \
        "$APPDIR/usr/share/icons/hicolor/${size}x${size}/apps/ai-assistant.png"
done

mkdir -p "$OUT_DIR"
OUT_FILE="$OUT_DIR/ai-assistant-${VERSION}-x86_64.AppImage"
ARCH=x86_64 "$APPIMAGETOOL" "$APPDIR" "$OUT_FILE"

echo
echo "Built $OUT_FILE"
echo "Run directly with: $OUT_FILE"
