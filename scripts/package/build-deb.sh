#!/usr/bin/env bash
# Builds a .deb package for ai-assistant (Development_plan Phase 20
# "Packaging" -- DEB).
#
# Usage: scripts/package/build-deb.sh [path-to-release-binary]
#   Defaults to target/release/ai-assistant (the normal `cargo build
#   --release` output location). The binary itself isn't built by this
#   script -- see docs/ROADMAP.md's Phase 20 notes for why `cargo build
#   --release -p app` can't run in this sandbox (the same Slint/Candle
#   toolchain wall as every phase since 2), and for how this script was
#   verified anyway (a stand-in binary, the same technique Phase 18's
#   throwaway harness used).
#
# Runtime Depends: below are a best-effort list based on what Slint and
# this workspace's other dependencies typically need (fontconfig,
# freetype, and either X11 or Wayland client libraries depending on
# Slint's backend) -- not confirmed by running `ldd` against a real
# binary, since one has never been built here. Re-generate this list
# with `dpkg-shlibdeps` (or just `ldd target/release/ai-assistant | grep
# "=>"`) once a real toolchain produces a real binary, before an actual
# release.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BINARY="${1:-target/release/ai-assistant}"
VERSION=$(grep -A2 '^\[workspace.package\]' Cargo.toml | grep '^version' | head -1 | sed -E 's/version = "(.*)"/\1/')
ARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
OUT_DIR="target/package"
STAGE="$OUT_DIR/deb-stage"

if [[ ! -f "$BINARY" ]]; then
    echo "error: binary not found at $BINARY -- build it first (cargo build --release -p app)" >&2
    exit 1
fi

echo "Building ai-assistant_${VERSION}_${ARCH}.deb from $BINARY"

rm -rf "$STAGE"
mkdir -p "$STAGE/DEBIAN"
mkdir -p "$STAGE/usr/bin"
mkdir -p "$STAGE/usr/share/applications"
mkdir -p "$STAGE/usr/share/doc/ai-assistant"
for size in 16 32 48 64 128 256 512; do
    mkdir -p "$STAGE/usr/share/icons/hicolor/${size}x${size}/apps"
done

install -m 755 "$BINARY" "$STAGE/usr/bin/ai-assistant"
install -m 644 packaging/shared/ai-assistant.desktop "$STAGE/usr/share/applications/ai-assistant.desktop"
for size in 16 32 48 64 128 256 512; do
    src="assets/icons/ai-assistant-${size}.png"
    if [[ -f "$src" ]]; then
        install -m 644 "$src" "$STAGE/usr/share/icons/hicolor/${size}x${size}/apps/ai-assistant.png"
    fi
done

# Debian's own copyright-file convention: license text plus a
# machine-readable stanza (DEP-5-ish, kept simple).
{
    echo "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/"
    echo "Upstream-Name: ai-assistant"
    echo "Source: https://github.com/lion/ai-assistant"
    echo
    echo "Files: *"
    echo "Copyright: 2026 Lion"
    echo "License: MIT OR Apache-2.0"
    echo
    echo "License: MIT"
    sed 's/^/ /; s/^ $/ ./' LICENSE-MIT
    echo
    echo "License: Apache-2.0"
    sed 's/^/ /; s/^ $/ ./' LICENSE-APACHE
} > "$STAGE/usr/share/doc/ai-assistant/copyright"

if [[ -f README.md ]]; then
    install -m 644 README.md "$STAGE/usr/share/doc/ai-assistant/README.md"
fi

installed_size=$(du -sk "$STAGE/usr" | cut -f1)
sed -e "s/__VERSION__/${VERSION}/" \
    -e "s/__ARCH__/${ARCH}/" \
    -e "s/__INSTALLED_SIZE__/${installed_size}/" \
    packaging/deb/control.template > "$STAGE/DEBIAN/control"

install -m 755 packaging/deb/postinst "$STAGE/DEBIAN/postinst"
install -m 755 packaging/deb/postrm "$STAGE/DEBIAN/postrm"

mkdir -p "$OUT_DIR"
OUT_FILE="$OUT_DIR/ai-assistant_${VERSION}_${ARCH}.deb"
dpkg-deb --build --root-owner-group "$STAGE" "$OUT_FILE"

echo
echo "Built $OUT_FILE"
echo "Inspect with: dpkg-deb --info $OUT_FILE && dpkg-deb --contents $OUT_FILE"
echo "Install with: sudo dpkg -i $OUT_FILE"
