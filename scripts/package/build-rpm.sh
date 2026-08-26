#!/usr/bin/env bash
# Builds an .rpm package for ai-assistant (Development_plan Phase 20
# "Packaging" -- RPM). See packaging/rpm/ai-assistant.spec's header
# comment for the overall approach.
#
# Usage: scripts/package/build-rpm.sh [path-to-release-binary]
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."

BINARY="${1:-target/release/ai-assistant}"
VERSION=$(grep -A2 '^\[workspace.package\]' Cargo.toml | grep '^version' | head -1 | sed -E 's/version = "(.*)"/\1/')
RPMBUILD_ROOT="target/package/rpmbuild"

if [[ ! -f "$BINARY" ]]; then
    echo "error: binary not found at $BINARY -- build it first (cargo build --release -p app)" >&2
    exit 1
fi

echo "Building ai-assistant-${VERSION} rpm from $BINARY"

rm -rf "$RPMBUILD_ROOT"
mkdir -p "$RPMBUILD_ROOT"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

cp "$BINARY" "$RPMBUILD_ROOT/SOURCES/ai-assistant"
cp packaging/shared/ai-assistant.desktop "$RPMBUILD_ROOT/SOURCES/ai-assistant.desktop"
for size in 16 32 48 64 128 256 512; do
    cp "assets/icons/ai-assistant-${size}.png" "$RPMBUILD_ROOT/SOURCES/ai-assistant-${size}.png"
done

{
    echo "This package is dual-licensed under the MIT License and the Apache"
    echo "License, Version 2.0. See LICENSE-MIT and LICENSE-APACHE in the"
    echo "source repository for the full text of each."
} > "$RPMBUILD_ROOT/SOURCES/copyright.txt"

cp packaging/rpm/ai-assistant.spec "$RPMBUILD_ROOT/SPECS/ai-assistant.spec"

rpmbuild --define "_topdir $(pwd)/${RPMBUILD_ROOT}" \
         --define "_ai_assistant_version ${VERSION}" \
         -bb "$RPMBUILD_ROOT/SPECS/ai-assistant.spec"

OUT=$(find "$RPMBUILD_ROOT/RPMS" -name '*.rpm' | head -1)
mkdir -p target/package
cp "$OUT" target/package/
echo
echo "Built $(basename "$OUT") -> target/package/$(basename "$OUT")"
echo "Inspect with: rpm -qip target/package/$(basename "$OUT") && rpm -qlp target/package/$(basename "$OUT")"
echo "Install with: sudo rpm -i target/package/$(basename "$OUT")"
