#!/usr/bin/env bash
#
# Downloads sing-box and xray-core binaries and renames them
# with the correct Tauri target-triple suffix.
#
# Usage:  ./download-cores.sh [target]
# Example: ./download-cores.sh x86_64-pc-windows-msvc
#
# If no target is given, auto-detects from `rustc`.

set -euo pipefail

SINGBOX_VERSION="1.12.18"
XRAY_VERSION="26.2.2"

BINDIR="$(cd "$(dirname "$0")" && pwd)/binaries"
mkdir -p "$BINDIR"

# Auto-detect target triple
if [ -n "${1:-}" ]; then
    TARGET="$1"
else
    TARGET=$(rustc -vV | grep host | cut -d' ' -f2)
fi

echo "Target: $TARGET"
echo "Output: $BINDIR"
echo ""

# ── Helpers ──────────────────────────────────────────

download() {
    local url="$1" dest="$2"
    echo "  Downloading $(basename "$dest")..."
    echo "  URL: $url"
    if command -v curl &>/dev/null; then
        curl -fsSL -o "$dest" "$url"
    elif command -v wget &>/dev/null; then
        wget -q -O "$dest" "$url"
    else
        echo "ERROR: curl or wget required" && exit 1
    fi
}

# ── Map target triple → download arch ────────────────

case "$TARGET" in
    x86_64-pc-windows-msvc|x86_64-pc-windows-gnu)
        SB_ARCH="windows-amd64"
        SB_EXT="zip"
        XRAY_ARCH="windows-64"
        XRAY_EXT="zip"
        EXE=".exe"
        ;;
    aarch64-pc-windows-msvc)
        SB_ARCH="windows-arm64"
        SB_EXT="zip"
        XRAY_ARCH="windows-arm64-v8a"
        XRAY_EXT="zip"
        EXE=".exe"
        ;;
    x86_64-unknown-linux-gnu|x86_64-unknown-linux-musl)
        SB_ARCH="linux-amd64"
        SB_EXT="tar.gz"
        XRAY_ARCH="linux-64"
        XRAY_EXT="zip"
        EXE=""
        ;;
    aarch64-unknown-linux-gnu|aarch64-unknown-linux-musl)
        SB_ARCH="linux-arm64"
        SB_EXT="tar.gz"
        XRAY_ARCH="linux-arm64-v8a"
        XRAY_EXT="zip"
        EXE=""
        ;;
    x86_64-apple-darwin)
        SB_ARCH="darwin-amd64"
        SB_EXT="tar.gz"
        XRAY_ARCH="macos-64"
        XRAY_EXT="zip"
        EXE=""
        ;;
    aarch64-apple-darwin)
        SB_ARCH="darwin-arm64"
        SB_EXT="tar.gz"
        XRAY_ARCH="macos-arm64-v8a"
        XRAY_EXT="zip"
        EXE=""
        ;;
    *)
        echo "ERROR: Unsupported target: $TARGET"
        exit 1
        ;;
esac

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# ── sing-box ─────────────────────────────────────────

echo "=== sing-box v${SINGBOX_VERSION} ==="
SB_URL="https://github.com/SagerNet/sing-box/releases/download/v${SINGBOX_VERSION}/sing-box-${SINGBOX_VERSION}-${SB_ARCH}.${SB_EXT}"
SB_ARCHIVE="$TMPDIR/singbox.${SB_EXT}"
download "$SB_URL" "$SB_ARCHIVE"

if [ "$SB_EXT" = "zip" ]; then
    unzip -q -o "$SB_ARCHIVE" -d "$TMPDIR/singbox"
else
    mkdir -p "$TMPDIR/singbox"
    tar xzf "$SB_ARCHIVE" -C "$TMPDIR/singbox"
fi

# Find the binary inside extracted dir
SB_BIN=$(find "$TMPDIR/singbox" -name "sing-box${EXE}" -type f | head -1)
if [ -z "$SB_BIN" ]; then
    echo "ERROR: sing-box binary not found in archive"
    exit 1
fi

cp "$SB_BIN" "$BINDIR/sing-box-${TARGET}${EXE}"
chmod +x "$BINDIR/sing-box-${TARGET}${EXE}"
echo "  ✓ $BINDIR/sing-box-${TARGET}${EXE}"
echo ""

# ── Xray-core ────────────────────────────────────────

echo "=== Xray-core v${XRAY_VERSION} ==="
XRAY_URL="https://github.com/XTLS/Xray-core/releases/download/v${XRAY_VERSION}/Xray-${XRAY_ARCH}.${XRAY_EXT}"
XRAY_ARCHIVE="$TMPDIR/xray.${XRAY_EXT}"
download "$XRAY_URL" "$XRAY_ARCHIVE"

mkdir -p "$TMPDIR/xray"
unzip -q -o "$XRAY_ARCHIVE" -d "$TMPDIR/xray"

XRAY_BIN=$(find "$TMPDIR/xray" -name "xray${EXE}" -type f | head -1)
if [ -z "$XRAY_BIN" ]; then
    echo "ERROR: xray binary not found in archive"
    exit 1
fi

cp "$XRAY_BIN" "$BINDIR/xray-${TARGET}${EXE}"
chmod +x "$BINDIR/xray-${TARGET}${EXE}"
echo "  ✓ $BINDIR/xray-${TARGET}${EXE}"
echo ""

echo "=== Done! ==="
echo "Binaries ready in $BINDIR:"
ls -lh "$BINDIR"
