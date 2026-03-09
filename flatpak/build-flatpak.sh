#!/bin/bash
set -euo pipefail

# Build NexVPN Flatpak locally
# Prerequisites:
#   sudo apt install flatpak flatpak-builder
#   flatpak install flathub org.gnome.Platform//47 org.gnome.Sdk//47
#   flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08
#   flatpak install flathub org.freedesktop.Sdk.Extension.node22//24.08

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_DIR/flatpak-build"
REPO_DIR="$PROJECT_DIR/flatpak-repo"

echo "=== Building NexVPN Flatpak ==="
echo "Project: $PROJECT_DIR"
echo ""

# Build and export to repo in one step
flatpak-builder \
  --force-clean \
  --user \
  --install-deps-from=flathub \
  --repo="$REPO_DIR" \
  "$BUILD_DIR" \
  "$SCRIPT_DIR/com.horusvpn.nexvpn.yml"

echo ""
echo "=== Build complete ==="
echo ""

# Create single-file bundle
echo "Creating .flatpak bundle..."
flatpak build-bundle "$REPO_DIR" "$PROJECT_DIR/nexvpn.flatpak" com.horusvpn.nexvpn

echo ""
echo "=== Done ==="
echo "Flatpak bundle: $PROJECT_DIR/nexvpn.flatpak"
echo ""
echo "Install with: flatpak install nexvpn.flatpak"
echo "Run with:     flatpak run com.horusvpn.nexvpn"
