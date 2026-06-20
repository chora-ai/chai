#!/usr/bin/env bash
# build-release.sh - Build and package chai release binaries.
#
# Usage:
#   ./scripts/build-release.sh <version> [system]
#
# Arguments:
#   version   Version string (e.g. 0.1.0)
#   system    Nix system triple (default: current host)
#
# Supported systems:
#   x86_64-linux    Native on x86_64 Linux
#   aarch64-linux   Requires binfmt emulation
#   aarch64-darwin  Must run on macOS ARM64 hardware
#
# Examples:
#   ./scripts/build-release.sh 0.1.0                    # build for current host
#   ./scripts/build-release.sh 0.1.0 x86_64-linux       # build for x86_64 Linux
#   ./scripts/build-release.sh 0.1.0 aarch64-linux      # build for aarch64 Linux (needs binfmt)
#
# Output:
#   dist/chai-v<version>-<system>.tar.gz
#   dist/chai-desktop-v<version>-<system>.tar.gz
#   dist/checksums-v<version>-<system>.txt

set -euo pipefail

# -- Args ---------------------------------------------------------------------

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <version> [system]" >&2
    exit 1
fi

VERSION="$1"
SYSTEM="${2:-}"

# Detect current system if not specified.
if [[ -z "$SYSTEM" ]]; then
    SYSTEM="$(nix eval --raw nixpkgs#system 2>/dev/null || nix eval --impure --raw --expr 'builtins.currentSystem')"
fi

# Validate system.
SUPPORTED_SYSTEMS="x86_64-linux aarch64-linux aarch64-darwin"
if ! echo "$SUPPORTED_SYSTEMS" | grep -qw "$SYSTEM"; then
    echo "error: unsupported system '$SYSTEM'" >&2
    echo "supported: $SUPPORTED_SYSTEMS" >&2
    exit 1
fi

# -- Paths --------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$REPO_ROOT/dist"

# -- Preflight ----------------------------------------------------------------

echo "chai release build"
echo "  version : $VERSION"
echo "  system  : $SYSTEM"
echo "  repo    : $REPO_ROOT"

# Verify we're on a clean tag or commit.
if git -C "$REPO_ROOT" describe --tags --exact-match HEAD &>/dev/null; then
    TAG="$(git -C "$REPO_ROOT" describe --tags --exact-match HEAD)"
    echo "  tag     : $TAG"
else
    TAG=""
    echo "  tag     : (not a tagged commit)"
fi

# Warn if aarch64-linux without binfmt.
if [[ "$SYSTEM" == "aarch64-linux" ]]; then
    if [[ ! -d /proc/sys/fs/binfmt_misc ]] || [[ ! -f /proc/sys/fs/binfmt_misc/aarch64-linux ]]; then
        echo "warning: aarch64-linux requires binfmt emulation." >&2
    fi
fi

# Warn if aarch64-darwin on Linux.
if [[ "$SYSTEM" == "aarch64-darwin" ]] && uname -r | grep -qi linux; then
    echo "error: cannot build aarch64-darwin on Linux. macOS hardware is required." >&2
    exit 1
fi

# -- Build --------------------------------------------------------------------

rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

build_package() {
    local pkg_name="$1"      # cli or desktop
    local bin_name="$2"      # chai or chai-desktop
    local flake_output="$3"  # cli or desktop (attribute name)

    echo "building $pkg_name ($bin_name) for $SYSTEM..."

    # Use the fully qualified attribute path (e.g. packages.aarch64-linux.cli)
    # instead of --system, which requires trusted-user status in the Nix daemon.
    local host_system
    host_system="$(nix eval --impure --raw --expr 'builtins.currentSystem')"
    local flake_attr
    if [[ "$SYSTEM" == "$host_system" ]]; then
        flake_attr="$flake_output"
    else
        flake_attr="packages.$SYSTEM.$flake_output"
    fi

    nix build "$REPO_ROOT#$flake_attr" --print-build-logs

    # Verify the binary exists.
    if [[ ! -f "$REPO_ROOT/result/bin/$bin_name" ]]; then
        echo "error: expected binary not found at result/bin/$bin_name" >&2
        exit 1
    fi

    # Package into tarball.
    local archive_name="${bin_name}-v${VERSION}-${SYSTEM}.tar.gz"
    tar -czf "$DIST_DIR/$archive_name" -C "$REPO_ROOT/result/bin" "$bin_name"
    echo "  packaged: dist/$archive_name"

    # Clean up result symlink.
    rm -f "$REPO_ROOT/result"
}

echo "=== CLI ==="
build_package "cli" "chai" "cli"

echo "=== Desktop ==="
build_package "desktop" "chai-desktop" "desktop"

# -- Checksums ----------------------------------------------------------------

echo "generating checksums..."

CHECKSUM_FILE="$DIST_DIR/checksums-v${VERSION}-${SYSTEM}.txt"
(cd "$DIST_DIR" && sha256sum "chai-v${VERSION}-${SYSTEM}.tar.gz" "chai-desktop-v${VERSION}-${SYSTEM}.tar.gz" > "$CHECKSUM_FILE")

echo "  written: dist/checksums-v${VERSION}-${SYSTEM}.txt"

# -- Summary ------------------------------------------------------------------

echo "=== Release assets for $SYSTEM ==="
ls -lh "$DIST_DIR"
