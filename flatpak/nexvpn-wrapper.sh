#!/bin/bash
# Wrapper script for NexVPN Flatpak
# Sets up environment so the app can find sidecar binaries

export NEXVPN_SIDECAR_DIR="/app/bin"
export PATH="/app/bin:$PATH"

# WebKitGTK DMA-BUF renderer causes "Protocol error 71" on some Wayland
# compositors inside the Flatpak sandbox — disable it as a safe fallback
export WEBKIT_DISABLE_DMABUF_RENDERER=1

# Ensure data directory exists
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/nexvpn"

exec /app/bin/nexvpn "$@"
