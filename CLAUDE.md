# NexVPN

Open-source cross-platform VPN client (Windows, macOS, Linux, Android).

## Tech Stack

- **Frontend**: React 18 + TypeScript + Vite
- **Backend**: Rust (Tauri v2)
- **VPN cores**: sing-box + Xray-core (bundled as sidecars)
- **Protocols**: VLESS, VMess, Shadowsocks, Trojan, Hysteria2, TUIC

## Build Commands

```bash
# Windows
npm run tauri build

# Android (arm64)
npm run tauri android build -- --target aarch64

# macOS (Apple Silicon)
npm run tauri build -- --target aarch64-apple-darwin

# Dev server
npm run tauri dev

# Type check only
npx tsc --noEmit

# Rust check only
cd src-tauri && cargo check
```

## Project Structure

```
src/                          # React frontend
  App.tsx                     # Main app, routing, auto-reconnect, keyboard shortcuts
  api/tauri.ts                # All Tauri invoke calls + TS types
  context/AppContext.tsx       # Global state (useReducer)
  components/
    home/                     # StatusPanel, TrafficPanel, ServerList, ServerCard, QuickConnect, AnnounceBanner, WorldMap
    subscriptions/            # SubList (compact/expandable cards), AddSubModal
    settings/                 # SettingsPage, PerAppVpn
    routing/                  # RoutingPage (domain rules, presets)
    stats/                    # StatsPage (dashboard, history, traffic chart)
    logs/                     # LogsPage (Core/App subtabs)
    ui/                       # Button, Modal, Spinner, Toast, ConfirmDialog, Icons, Flag
    onboarding/               # OnboardingOverlay
  i18n/translations.ts        # EN/RU translations
  themes/themes.ts            # 7 color themes
  utils/                      # countryUtils, confirm

src-tauri/                    # Rust backend
  src/
    commands.rs               # All Tauri commands, AppContext, humanize_core_error
    lib.rs                    # Tauri app setup, plugins, deep links
    core/
      manager.rs              # CoreManager — start/stop cores, health check, logs, traffic stats
      singbox.rs              # sing-box config generation
      xray.rs                 # Xray config generation
    proxy/
      models.rs               # Server, Subscription, Settings, AppState, etc.
      subscription.rs         # Fetch subscription URL, parse headers (metadata, announce, traffic)
      link_parser.rs          # Parse vless://, vmess://, ss://, trojan://, hy2:// links
    system/
      hwid.rs                 # Device fingerprint (HWID, platform, model)
      proxy_setter.rs         # Windows/macOS system proxy toggle
    testing/
      ping.rs                 # TCP ping, VPN ping, auto-select
  binaries/                   # Sidecar binaries (sing-box, xray, wintun.dll)
  tauri.conf.json             # Tauri config (version, sidecars, NSIS)
```

## Key Architecture

- **Subscription metadata**: Headers parsed — `Profile-Update-Interval`, `Subscription-Userinfo`, `Support-Url`, `Announce`, `Subscription-Refill-Date`
- **Auto-update**: Frontend timer based on `update_interval` from subscription headers
- **TUN mode**: sing-box has native TUN. Xray — TUN not supported on desktop (only proxy mode)
- **Android VPN**: VpnService + tun2socks on Java side, Rust just manages core process
- **Anti-detection**: Random ports, authenticated local proxies, random Clash API secret
- **State**: Persisted to `state.json` in app data dir (`AppData/Roaming/nexvpn` on Windows)

## Conventions

- Use `python` not `python3` on Windows
- GitHub release titles = version only (e.g. "v1.4.2"), details in notes body
- APK signing: `sign-apk.bat` or manual zipalign + apksigner with `nexvpn-release.keystore`
- CSS uses `var(--danger)`, `var(--warning)`, `var(--accent)` — never hardcode hex colors
- Icons: Feather-style SVG in `Icons.tsx` — no emoji in UI code
- Confirm dialogs: use `showConfirm()` from `utils/confirm.ts` (works on macOS Tauri)
- Error handling in toasts: `e instanceof Error ? e.message : String(e)`
- Translations: always add both EN and RU in `translations.ts`
- App logs: use `app_log()` / `ctx.app_logs` for user-visible logs in App tab
