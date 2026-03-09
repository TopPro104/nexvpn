#!/bin/bash
# Wrapper script for NexVPN Flatpak
# Sets up environment so the app can find sidecar binaries

export NEXVPN_SIDECAR_DIR="/app/bin"
export PATH="/app/bin:$PATH"

# Ensure data directory exists
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/nexvpn"

exec /app/bin/nexvpn "$@"
