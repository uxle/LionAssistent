#!/usr/bin/env bash
# Builds a self-contained portable tarball for ai-assistant
# (Development_plan Phase 20 "Packaging" -- the "portable" option from
# the project's own directory layout, alongside DEB/RPM/AppImage/
# Flatpak/Snap). No installation, no package manager, no root: extract
# and run `./ai-assistant/run.sh`. The simplest of the six formats, and
# correspondingly the easiest to verify completely -- no external
# packaging tool involved at all, just `tar`.
#
# Usage: scripts/package/build-portable.sh [path-to-release-binary]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BINARY="${1:-target/release/ai-assistant}"
VERSION=$(grep -A2 '^\[workspace.package\]' Cargo.toml | grep '^version' | head -1 | sed -E 's/version = "(.*)"/\1/')
ARCH=$(uname -m)
OUT_DIR="target/package"
STAGE="$OUT_DIR/ai-assistant-${VERSION}-${ARCH}-portable"

if [[ ! -f "$BINARY" ]]; then
    echo "error: binary not found at $BINARY -- build it first (cargo build --release -p app)" >&2
    exit 1
fi

echo "Building ai-assistant-${VERSION}-${ARCH}-portable.tar.gz from $BINARY"

rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/share/icons" "$STAGE/share/applications" "$STAGE/share/doc"

install -m 755 "$BINARY" "$STAGE/bin/ai-assistant"
install -m 644 packaging/shared/ai-assistant.desktop "$STAGE/share/applications/ai-assistant.desktop"
install -m 644 packaging/shared/com.lionos.AiAssistant.metainfo.xml "$STAGE/share/doc/"
for size in 16 32 48 64 128 256 512; do
    install -m 644 "assets/icons/ai-assistant-${size}.png" "$STAGE/share/icons/"
done
install -m 644 LICENSE-MIT LICENSE-APACHE "$STAGE/share/doc/"
if [[ -f README.md ]]; then
    install -m 644 README.md "$STAGE/share/doc/"
fi

cat > "$STAGE/run.sh" << 'RUNSCRIPT'
#!/usr/bin/env bash
# Runs the bundled ai-assistant binary regardless of the current working
# directory -- resolves everything relative to this script's own real
# location, the same technique packaging/appimage/AppRun uses.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "$HERE/bin/ai-assistant" "$@"
RUNSCRIPT
chmod +x "$STAGE/run.sh"

cat > "$STAGE/README-portable.txt" << README
ai-assistant ${VERSION} -- portable build

No installation needed. Run:

    ./run.sh

To install desktop integration (menu entry + icon) manually instead of
using this portable form, copy:
  share/applications/ai-assistant.desktop -> ~/.local/share/applications/
  share/icons/ai-assistant-256.png        -> ~/.local/share/icons/hicolor/256x256/apps/ai-assistant.png
and edit the Exec= line in the copied .desktop file to the full path of
this directory's bin/ai-assistant.
README

mkdir -p "$OUT_DIR"
OUT_FILE="$OUT_DIR/ai-assistant-${VERSION}-${ARCH}-portable.tar.gz"
tar czf "$OUT_FILE" -C "$OUT_DIR" "$(basename "$STAGE")"

echo
echo "Built $OUT_FILE"
echo "Use with: tar xzf $(basename "$OUT_FILE") && ./ai-assistant-${VERSION}-${ARCH}-portable/run.sh"
