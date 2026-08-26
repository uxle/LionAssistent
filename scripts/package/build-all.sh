#!/usr/bin/env bash
# Runs every packaging format this phase supports building for real
# (DEB, RPM, AppImage, portable tarball) against one already-built
# release binary. Flatpak and Snap are deliberately not included here --
# see docs/ROADMAP.md's Phase 20 notes for why (Flathub and snapcraft
# itself are both outside this sandbox's network allowlist, confirmed
# empirically, not assumed); build those with
# `flatpak-builder`/`snapcraft` directly once run somewhere that has
# them.
#
# Usage: scripts/package/build-all.sh [path-to-release-binary]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BINARY="${1:-target/release/ai-assistant}"

if [[ ! -f "$BINARY" ]]; then
    echo "error: binary not found at $BINARY -- build it first (cargo build --release -p app)" >&2
    exit 1
fi

./scripts/package/render-icons.sh
./scripts/package/build-deb.sh "$BINARY"
./scripts/package/build-rpm.sh "$BINARY"
if command -v appimagetool >/dev/null 2>&1 || [[ -n "${APPIMAGETOOL:-}" ]]; then
    ./scripts/package/build-appimage.sh "$BINARY"
else
    echo "skipping AppImage: appimagetool not found (set \$APPIMAGETOOL or put it on PATH)"
fi
./scripts/package/build-portable.sh "$BINARY"

echo
echo "All built packages are in target/package/:"
ls -la target/package/*.deb target/package/*.rpm target/package/*.AppImage target/package/*.tar.gz 2>/dev/null || true
