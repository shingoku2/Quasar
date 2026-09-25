# Quasar

A desktop app (Tauri 2: Rust backend, React + TypeScript frontend) for managing remote infrastructure: SSH terminals, SFTP, tunnels, monitoring and alerts, network discovery, scheduled tasks, and an encrypted credential vault. Windows, macOS and Linux.

## Features

**Credential vault**
- Master-password vault. Credentials are encrypted with AES-256-GCM under a key derived with Argon2id + HKDF; the key never touches disk.
- SSH (password or key), RDP, database, API and other credentials. A credential can be bound to a host, and then it only works against that host.
- Auto-lock (1-1440 minutes) and an unlock lockout that survives restarts.
- SSH host-key pinning with MITM warnings for changed keys, and an audit log of vault events.
- Revealing a password or weakening a trust decision asks through a native OS dialog.

**Remote access**
- SSH terminal (xterm.js) with several concurrent sessions, and phase-by-phase connect errors (DNS, TCP, handshake).
- SFTP browse, upload and download (password credentials). Local files are picked with the native dialog.
- SSH local port forwarding (tunnels).
- RDP launch through the system client.
- Tailscale: lists tailnet peers from the local `tailscale` CLI (no API key), adds them as hosts, and connects to Tailscale SSH peers by tailnet identity.

**Automation**
- Cron-scheduled tasks (6-field cron): SSH command, SFTP upload or SFTP download, with last-run status and output, and "Run now".

**Monitoring**
- Live local metrics: CPU, memory, per-disk usage, network, load.
- Remote host health: ping, plus SSH metrics on Linux targets.
- Alert rules with thresholds and cooldowns, persisted and edited in the Monitoring view. Triggered and recovered alerts appear in the dashboard feed.

**Discovery**
- CIDR scan (13 common ports) with device classification, service detection and reverse DNS.
- mDNS discovery, an interactive topology graph, and persistent discovered-host tracking.

**Other**
- Optional local AI assistant (Ollama).
- Dark and light themes with an accent color.
- Signed auto-updates.

## Getting started

### Prerequisites

- Node.js 24.15 (`.nvmrc`) and npm 12
- Rust via rustup. `rust-toolchain.toml` pins the toolchain (1.98.1), and the minimum supported version is 1.95.
- Linux only: WebKitGTK and friends, e.g. on Debian/Ubuntu:
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libssl-dev
  ```
- Optional: the `tailscale` CLI (Tailscale panel) and Ollama (AI assistant).

### Run

```bash
git clone https://github.com/shingoku2/Quasar.git
cd Quasar
npm install
npm run tauri        # dev app: Vite on :1420 + the Rust backend
```

`npm run tauri` runs `scripts/tauri-dev.js`, which sets `CARGO_TARGET_DIR=src-tauri/target` and starts `tauri dev`. On first launch the app asks you to set a master password.

### Build

```bash
npx tauri build      # installers in src-tauri/target/release/bundle/
```

Release builds, signing and the updater are described in [`docs/RELEASE_SIGNING.md`](docs/RELEASE_SIGNING.md).

### Test

```bash
npm test                                              # frontend (Vitest)
npm run coverage                                      # with the coverage floor CI enforces
npx tsc --noEmit
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

## Documentation

| Doc | What's in it |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | Contributor and agent guide: commands, conventions, security invariants (AGENTS.md points here) |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | IPC commands and events, vault concurrency, security decisions, background work |
| [`docs/SCHEMA.md`](docs/SCHEMA.md) | Database tables, columns and migration rules |
| [`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md) | Threat model for the vault |
| [`docs/CORE_WORKFLOWS.md`](docs/CORE_WORKFLOWS.md) | User workflows, step by step |
| [`docs/style/`](docs/style/) | Code style guides |
| [`SECURITY.md`](SECURITY.md) | Reporting vulnerabilities; accepted dependency risks |
| [`CHANGELOG.md`](CHANGELOG.md) | What changed, by date |
| [`AUDIT.md`](AUDIT.md) | The September 2026 full audit |
| [`docs/archive/`](docs/archive/) | Historical reports and plans |

## Contributing

Branch from `master`, follow [`CLAUDE.md`](CLAUDE.md) and the style guides, add tests with your change, and open a pull request. CI runs typecheck, tests with coverage, clippy, cargo test, dependency audits and a build on all three platforms.

## License

MIT. See [LICENSE](LICENSE).
