# NexVPN

Open-source VPN client built with Tauri, React and Rust. Supports multiple proxy protocols via **sing-box** and **Xray-core** engines.

**[Russian version / Русская версия](README_RU.md)**

## Screenshots

### Windows
![Windows](desktop.jpg)

### macOS
![macOS](macos.jpg)

### Android
<img src="phone.jpg" width="300" alt="Android" />

## Features

- **Multi-protocol** — VLESS, VMess, Shadowsocks, Trojan, Hysteria2, TUIC
- **Dual core** — switch between sing-box and Xray-core in one click
- **Subscriptions** — import and auto-update subscription URLs
- **Link import** — paste `vless://`, `vmess://`, `ss://`, `trojan://` links directly
- **Deep links** — open `nexvpn://import/URL` to add subscriptions from browser
- **Routing rules** — domain-based rules (proxy / direct / block) with quick presets for ads blocking and regional bypass
- **Split tunneling** — choose default route: proxy all traffic or only selected domains (Direct All mode)
- **System Proxy & TUN mode** — system HTTP proxy or full TUN VPN (captures all traffic)
- **Admin elevation** — one-click "Run as Administrator" button for TUN mode (Windows UAC / macOS sudo prompt)
- **Onboarding** — interactive guided tour for first-time users with language selection
- **TCP ping** — single and bulk server latency testing, auto-select best server
- **Quick connect** — recently used and recommended servers for one-tap connection
- **World map** — interactive map visualization of server locations
- **Themes** — 7 color themes (Dark, Light, Midnight Blue, Cyberpunk, Aurora, Sunset, Matrix)
- **Styles** — Default, Modern Minimal, Glassmorphism, Neon Glow
- **Animations** — None, Smooth, Energetic (configurable, CSS-only)
- **Statistics** — connection history, traffic totals, live speed graph with overview/traffic/history tabs
- **Logs** — real-time log viewer with level filtering (All / Errors / Warnings)
- **i18n** — English and Russian
- **HWID** — optional device fingerprint for panel-based subscriptions
- **Keyboard shortcuts** — number keys (1-6) for navigation, Shift+D connect/disconnect, Shift+F search
- **Auto-reconnect** — automatically reconnects if the VPN connection drops unexpectedly
- **Adaptive UI** — responsive layout with floating sidebar on desktop and bottom navigation bar on mobile
- **Cross-platform** — Windows, macOS, Android from a single codebase
- **Lightweight** — single binary, no Electron, ~16 MB app size

## Download

Pre-built installers for Windows (x64), macOS (Apple Silicon) and Android (APK) are available on the [Releases](../../releases) page.

## Building from source

### Requirements

- [Rust](https://rustup.rs/) 1.70+
- [Node.js](https://nodejs.org/) 18+
- Tauri CLI: `cargo install tauri-cli`
- For Android: Android SDK, NDK and Java (Android Studio recommended)

### 1. Clone the repository

```bash
git clone https://github.com/TopPro104/nexvpn.git
cd nexvpn
```

### 2. Install dependencies

```bash
npm install
```

### 3. Download proxy cores

The script downloads sing-box and Xray-core binaries and places them in `src-tauri/binaries/` with the correct Tauri target-triple names:

```bash
# Linux / macOS
chmod +x download-cores.sh
./download-cores.sh

# Windows (Git Bash)
bash download-cores.sh
```

The script auto-detects your platform via `rustc`. You can specify a target explicitly:

```bash
./download-cores.sh x86_64-pc-windows-msvc
```

For TUN mode on Windows, also place `wintun.dll` in `src-tauri/binaries/`.

### 4. Run or build

```bash
# Development
cargo tauri dev

# Production build — Windows (MSI + NSIS installer)
cargo tauri build

# Production build — Android (unsigned APK)
npx tauri android build --apk
```

## Project structure

```
nexvpn/
├── src/                        # React frontend
│   ├── api/tauri.ts            # Tauri IPC bindings
│   ├── components/
│   │   ├── home/               # StatusPanel, ServerList, TrafficPanel, QuickConnect, WorldMap
│   │   ├── layout/             # Layout (top bar + sub-tabs), Sidebar
│   │   ├── settings/           # SettingsPage (style / vpn / other tabs)
│   │   ├── subscriptions/      # SubList
│   │   ├── routing/            # RoutingPage
│   │   ├── stats/              # StatsPage (overview / traffic / history)
│   │   ├── logs/               # LogsPage (with level filtering)
│   │   ├── onboarding/         # OnboardingOverlay
│   │   └── ui/                 # Icons, Toast, shared UI
│   ├── context/AppContext.tsx   # Global state (useReducer)
│   ├── hooks/                  # useTheme, useStatus, useTraffic
│   ├── i18n/translations.ts    # EN/RU translations
│   ├── themes/themes.ts        # 7 color theme definitions
│   └── index.css               # All styles (no CSS framework)
├── src-tauri/src/
│   ├── lib.rs                  # Tauri app setup
│   ├── commands.rs             # IPC commands (frontend <-> Rust)
│   ├── core/
│   │   ├── manager.rs          # Core process lifecycle
│   │   ├── singbox.rs          # sing-box config generation
│   │   └── xray.rs             # Xray config generation
│   ├── proxy/
│   │   ├── models.rs           # Data models (Server, Settings, etc.)
│   │   ├── link_parser.rs      # Protocol link parser
│   │   └── subscription.rs     # Subscription fetcher
│   ├── system/
│   │   ├── proxy_setter.rs     # Windows system proxy control
│   │   └── hwid.rs             # HWID generation for panel auth
│   └── testing/
│       └── ping.rs             # TCP latency testing
├── download-cores.sh           # Core binary downloader
├── sign-apk.bat                # Android APK signing script
├── LICENSE
└── README.md
```

## Running on macOS

Open the `.dmg`, drag NexVPN to Applications, then launch normally. In **Proxy mode** (default) no extra permissions are needed.

For **TUN mode** (captures all traffic), the app needs elevated privileges. You can either:
- Click the **"Run as Administrator"** button in Settings → VPN Mode → TUN, or
- Launch from terminal:
```bash
sudo /Applications/NexVPN.app/Contents/MacOS/NexVPN
```

> **Note:** On first launch macOS may block the app. Go to **System Settings → Privacy & Security** and click **"Open Anyway"**.

## Running on Android

Install the APK directly. The app uses a responsive mobile layout with a bottom navigation bar. All features work the same as on desktop.

For TUN mode, Android will prompt you to allow VPN permissions on first connect.

## Community

Join our Telegram group for updates, support, and discussion:

[![Telegram](https://img.shields.io/badge/Telegram-NexVPN-2CA5E0?logo=telegram&logoColor=white)](https://t.me/NexVPNcom)

## License

[GNU GPLv3](LICENSE) — free to use, modify, and distribute.
This software is provided under the GNU General Public License v3.0.
See LICENSE for details.

## Publisher

HorusVPN
