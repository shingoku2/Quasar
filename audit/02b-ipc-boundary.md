# Phase 2B: IPC boundary and Tauri config audit (EDW-20)

Date: 2026-09-24 · Audited commit: `97dd84a`. The working tree HEAD is `6665448`, a merge that adds only `audit/` files; `git diff 97dd84a HEAD -- src src-tauri` is empty, so the source is identical. Baseline data (tests, coverage, dependency versions) comes from `audit/00-recon.md` / `audit/01-baseline.md`, which were recorded at `fcc58d2`. Every file:line below was re-read on the current tree.

## Threat model used

This is a single-user desktop app. It loads only its own bundled frontend: one window `main`, no remote URLs, no iframes. The attackers considered are:

1. **A malicious remote party.** That means an SSH/SFTP server, a host on a scanned LAN (hostnames, mDNS names), or a local Ollama model's output. It can only reach the webview through data that gets rendered.
2. **A compromised webview.** An attacker who gets script execution in the webview has full IPC access, because every one of the 73 commands is callable from `main`. This audit found **no route from (1) to (2)** (see "Checked and clean"). So every "webview-abuse" finding below is **defense in depth**, and severities are set to match: nothing is High or Critical, because no working XSS vector exists today.

Local attackers with filesystem access are out of scope for this phase.

## Scope & method

- **Commands.** The command list was re-derived from `#[tauri::command]` in `src-tauri/src/lib.rs` (70) and `src-tauri/src/ssh.rs` (3), for 73 in total, and checked against `generate_handler!` (`lib.rs:1957-2031`): the two sets are identical. Each handler was read for five things: input validation, whether it needs the vault unlocked, the error path, what the callee does with the inputs, and whether a webview could abuse it. Callees read: `ssh.rs`, `ssh_auth.rs`, `launcher.rs`, `sftp.rs` (host-key handler), `ssh_exec.rs` (host-key handler), `ssh_tunnel.rs` (bind address), `scheduler.rs` (task execution and credential resolution), `vault.rs` (settings, auto-lock, unlock errors), `vault/ssh_keys.rs`, `vault/credentials.rs` (audit events, search), `vault/audit.rs`, `host_tracker.rs` (search), `monitoring.rs` (rules, history), `errors.rs`, `validation.rs`, `ai.rs`.
- **Frontend usage.** Call sites were re-counted with a grep for `invoke('…')`: 63 distinct literal names, plus `upsert_saved_host`/`update_saved_host` through the ternary in `AddHostDialog.tsx`, for 65. The 8 never-invoked commands match recon §3.2.
- **Capabilities.** Read `src-tauri/capabilities/default.json` and `src-tauri/build.rs`. The permission-set definitions were checked in the actual crate sources: tauri 2.11.5, tauri-plugin-opener 2.5.4, -dialog 2.7.2, -process 2.3.1 and -updater 2.10.1, all downloaded from crates.io into the session scratchpad (not the repo). Versions come from `Cargo.lock`.
- **CSP.** Read `tauri.conf.json`, `index.html` and `vite.config.ts`. Grepped the frontend for `fetch(`, `WebSocket`, `XMLHttpRequest`, `EventSource` and localhost URLs.
- **XSS sinks.**
  - Grepped `src/` for `dangerouslySetInnerHTML`, `innerHTML`, `outerHTML`, `insertAdjacentHTML`, `document.write`, `eval(`, `new Function`. There were zero hits outside tests.
  - Read `TerminalComponent.tsx`, `AIAssistant.tsx` rendering, `NetworkTopologyView.tsx` node construction and `SshFileManager.tsx` path handling.
  - Checked the installed third-party code directly (tarballs fetched to the scratchpad): `@xterm/xterm` 6.0.0 `lib/xterm.mjs` (OSC 8 handler, OSC 52) and `vis-network` 10.1.2 `peer/esm/vis-network.js` (popup `setText`).
- **Events.** Listed every `emit(` in Rust (19 sites) and every `listen` in the frontend, and grepped for additional windows or webviews.
- **Not run.** Nothing was built, tested or installed. `npx knip`/`gitleaks` were not needed for this phase.

## Findings

| ID | Severity | file:line | Evidence | Impact | Fix | Effort (S/M/L) |
|---|---|---|---|---|---|---|
| IPC-001 | Medium | `src-tauri/src/validation.rs:145-160`; `lib.rs:1555-1556` (`sftp_upload_file`), `lib.rs:1586-1587` (`sftp_download_file`), `lib.rs:839-846` / `880-887` (scheduled SFTP tasks), `lib.rs:1703` (`export_database`), `lib.rs:1728` (`import_database`) | `validate_path` only rejects empty strings, NUL and `..` components, so any absolute path passes. The native dialog (`SshFileManager.tsx:117,157`, `SettingsView.tsx`) picks paths only in the UI. The backend never checks that a path came from a dialog. `scheduler.rs:405-460` passes `local_path` straight to `sftp::upload_file`/`download_file` on every cron tick. | A compromised webview gets arbitrary local file **read** (for example `sftp_upload_file` of `~/.ssh/id_ed25519` to an attacker SFTP server) and arbitrary file **write** (`sftp_download_file` into `~/.bashrc`, `~/.ssh/authorized_keys`, an autostart entry). That is local code execution. A scheduled `sftp_download` task makes it persistent. Combined with IPC-002 (trusting the attacker's host key), no user interaction is needed. | Stop accepting raw paths for local I/O. Either open the dialog from Rust (`tauri_plugin_dialog::DialogExt` in the command) and use the path it returns, or record the paths returned by dialogs in a backend allowlist and require `local_path` to be in it. For scheduled SFTP tasks, confine `local_path` to a user-chosen base directory saved at task creation. | M |
| IPC-002 | Medium | `lib.rs:1413-1444` (`trust_ssh_host_key`), `lib.rs:1482-1496` (`update_ssh_host_trust`), `ssh.rs:81-111`, `vault/ssh_keys.rs:178-245` | `trust_ssh_host_key` stores whatever `fingerprint`/`key_bytes` the caller passes for any host/port, then resolves `request_id`. The backend already computed the real fingerprint for that request (`ssh.rs:69-70`) but doesn't keep it in `HostKeyApprovalState` (`ssh.rs:13-15` stores only a oneshot sender), so it can't check the two match. `update_ssh_host_trust` can flip any stored key to `trusted`. There is no audit-log write anywhere in `ssh_keys.rs`. | Host-key trust is the only MITM protection on the non-interactive paths: `sftp.rs:47-57`, `ssh_exec.rs:43-55` (scheduler, monitoring) and `ssh_tunnel.rs`. A compromised webview can pre-seed trust for an attacker's key on any host, which silently redirects scheduled tasks, monitoring polls and SFTP (sending passwords, see IPC-003). No audit trail is left. | Store `(host, port, fingerprint, key_bytes)` in the pending map keyed by `request_id`. `trust_ssh_host_key` should take only `request_id` + permanence and persist the backend-computed values. Drop the fingerprint and key params from the IPC surface. Write `host_key_trust`/`host_key_trust_change` audit events. | S |
| IPC-003 | Medium | `lib.rs:219-242` (`connect_ssh`), `lib.rs:284-307` (`start_ssh_tunnel`), `lib.rs:1029-1060` (`set_host_monitoring_credential`), `lib.rs:998-1013` (`get_remote_hosts_health`), `scheduler.rs:361-388` | Any `credential_id` can be used against any host. The credential's own `host`/`port` (stored, returned in `CredentialSummary`, `vault/credentials.rs:30-40`) is never compared with the connection target. `set_host_monitoring_credential` binds any credential to any host with no existence or type check. After that, the 30 s monitoring poll logs in with it automatically. | With password auth the SSH server receives the plaintext password. A compromised webview can therefore exfiltrate every vault password without ever calling `reveal_credential_password`: bind the credential to an attacker host, trust its key (IPC-002), and wait for the poll. This also turns a user who picks the wrong credential into a password leak to the wrong server. | If `credential.host` is set, refuse to use the credential against a different host/port, or require an explicit "use for any host" flag on the credential. Validate that `host_id`/`credential_id` exist in `set_host_monitoring_credential`. | M |
| IPC-004 | Medium | `src/components/vault/VaultSettings.tsx:188-205`; `src-tauri/src/vault.rs:27,35,534,744-758` | The UI offers "Require master password for credential access: Prompt for master password each time a credential is accessed". The backend never reads `require_password_on_credential_use`: a grep finds only the struct field, the default, and `get_settings` echoing the in-memory value. `update_settings` persists only `auto_lock_timeout` (`vault.rs:748-755`), so the flag is also lost on restart. | This security control is advertised but not implemented. Users who enable it believe reveal, SSH connect and scheduled tasks require re-authentication, and they don't. | Either implement it (step-up check inside `credential_access()` for user-initiated reveal/connect, persisted in `vault_settings`) or remove the toggle. | S (remove) / M (implement) |
| IPC-005 | Low | `lib.rs:1303-1317` (`reveal_credential_password`); `vault/credentials.rs:320-331` | Reveal returns the plaintext with no step-up and no rate limit. It is audited as the same `credential_access` event (`credentials.rs:328`) that every `get_credential` view, SSH connect, tunnel, monitoring poll and scheduled run also writes. | Bulk dumping (`list_credentials` then `reveal` in a loop) is indistinguishable in the audit log from normal use. The CLAUDE.md claim that decrypted passwords only cross IPC through this command is true (`CredentialFrontendView` has no secret fields), but nothing narrows the command itself. | Log a distinct `credential_reveal` event. Route reveal through the IPC-004 step-up once it exists. | S |
| IPC-006 | Low | `lib.rs:1229-1238` (`update_vault_settings`); `vault.rs:744-758`, `vault.rs:459` | The whole `VaultSettings` struct from the webview replaces `inner.settings`, including `vault_initialized`, with no server-side bounds. The UI's 1-1440 range exists only in HTML (`VaultSettings.tsx:152-153`). `auto_lock_timeout_minutes * 60` is an unchecked `u64` multiply, and `Cargo.toml` sets no `overflow-checks`. The command isn't vault-gated, so it works while the vault is locked. | A huge value effectively disables auto-lock. In debug builds the overflow panics inside the auto-lock task (`lib.rs:1871-1886`), killing it for the rest of the session. In release it wraps to an arbitrary timeout. | Accept only the timeout field, clamp it to `1..=1440`, and use `checked_mul`. Consider requiring the vault to be unlocked to change security settings. | S |
| IPC-007 | Low | `src-tauri/tauri.conf.json` `app.security.csp` | `connect-src 'self' ws://localhost:* http://localhost:*` is shipped in production. The webview makes no network calls: zero `fetch`/`WebSocket`/`XMLHttpRequest`/`EventSource` in `src/`. Ollama is called from Rust (`ai.rs:8,14,33`, `Ollama::default()`). Only Vite HMR in dev needs localhost (`vite.config.ts:17-26`, ports 1420/1421). `img-src asset:` is also unused, since Cargo.toml doesn't enable tauri's `protocol-asset` feature and no asset scope is configured. | An XSS could reach any localhost service. That includes SSH tunnels bound to `127.0.0.1:<local_port>` (`ssh_tunnel.rs:185`), which forward into remote internal networks, and the local Ollama HTTP API. | Move the localhost sources into `app.security.devCsp` and ship `connect-src 'self'` (Tauri adds its IPC origins itself; UNVERIFIED on 2.11.5, confirm in a built app). Drop `asset:` from `img-src`. | S |
| IPC-008 | Low | `src-tauri/capabilities/default.json` | (a) `core:default` already contains `core:event:default` = listen/unlisten/emit/emit_to (tauri 2.11.5 `build.rs:34-40`, `449-470`; `permissions/event/autogenerated/reference.md:7-10`). The explicit `core:event:allow-listen`/`allow-emit` entries are redundant, and removing them wouldn't revoke emit. The frontend never calls `emit` (grep: 0). (b) `opener:default` = `allow-open-url` (http/https/mailto/tel) + `allow-reveal-item-in-dir` (unscoped). The frontend never imports the opener; its only use is the plugin's injected click handler for `<a target="_blank">` (`AIAssistant.tsx:229`, opener `init-iife.js`). (c) `dialog:default` = message/save/open, which is used. `updater:default` + `process:allow-restart` are used by `useUpdater.ts:2-3,65`, and updates are minisign-verified. | This isn't least privilege. A compromised webview gets extra primitives: open any http(s) URL in the default browser, reveal any path in the file manager, and emit spoofed app events such as a fake `ssh-host-key-verification` or `vault-auto-locked`. | Replace `core:default` with the specific core sets the app uses, minus `core:event:allow-emit`/`allow-emit-to`. Replace `opener:default` with `opener:allow-open-url` scoped to `https://ollama.com/*`, or drop the opener and the link. | S |
| IPC-009 | Low | `lib.rs:1726-1798` (`import_database`), `lib.rs:1701-1724` (`export_database`), `lib.rs:153-179` | Import replaces the whole DB in place from any path: vault hash, credentials, known-host trust, scheduled tasks, saved hosts. There is no vault-unlock requirement, no native confirmation, and a looser "legacy signature" acceptance path. Export writes a SQLite backup to any path, and a backup onto another app's existing SQLite file overwrites it. | Chained with IPC-001 (drop an attacker DB via `sftp_download_file`), a compromised webview can install attacker-trusted host keys and cron tasks in one call. In-memory state (vault key, scheduler, alert rules) isn't reset after import. | Require the vault to be unlocked, and confirm in native UI (Rust-side `dialog().message(...)`) for import. Only accept paths from backend-opened dialogs (IPC-001). Refuse to export over an existing file that isn't a Quasar DB. | S |
| IPC-010 | Low | `src-tauri/build.rs:1-3`; dead commands at `lib.rs:347, 501, 512, 815, 1111, 1135, 1530, 1622` | `tauri_build::build()` declares no app manifest, so all 73 commands are implicitly allowed for any window granted a capability: there is no per-command ACL. 8 registered commands are never invoked by the frontend (all re-verified with grep: 0 non-test hits): `get_metrics_history`, `get_alert_history`, `launch_ssh_external`, `get_host_details`, `search_discovered_hosts`, `get_scheduled_task`, `get_audit_log_count`, `sftp_remote_exists`. | This is attack surface with no product value. `launch_ssh_external` spawns OS processes (injection-safe, see Clean). `sftp_remote_exists` is an authenticated remote-file oracle. The rest are read-only DB queries. | Unregister the dead commands, or gate them behind `#[cfg(debug_assertions)]`. Adopt `tauri_build::try_build(Attributes::new().app_manifest(AppManifest::new().commands(&[..])))` and list `allow-<cmd>` entries in the capability, so new commands are denied by default. | S |
| IPC-011 | Low | `lib.rs:823-861`, `863-903`, `911-919`; `scheduler.rs:385-386`, `scheduler.rs:465` | Remote command execution is by design. The only gate is that the vault must be unlocked when the task has a credential. Tasks without a credential run with empty auth, which `ssh_auth.rs` turns into `none` auth. `task_type` isn't validated (any unknown value falls through to SSH exec at `scheduler.rs:465`), nor are `host_id` existence, `name` or `command` length. Creating, updating or running a task writes no audit event. | A webview compromise can plant persistent cron jobs on every saved host, including hosts that accept `none` auth such as Tailscale SSH, and nothing shows up in the security audit log. | Validate `task_type` as an enum, check that `host_id` exists, write `scheduled_task_create/update/run` audit events, and optionally require native confirmation on create/update. | S |
| IPC-012 | Info | `lib.rs:1206` + `errors.rs:14`; raw-error paths: `lib.rs:1043`, `1693`, `1745`, `1765-1784`, `570/583` (via `997`), `scheduler.rs:425-450` | **Over-sanitized:** `unlock_vault` maps every error, including "Vault is locked out. Try again in N seconds" (`vault.rs:317-319`) and the lockout-escalation messages (`vault.rs:366-372`), to "Vault operation failed", so the user never sees the lockout. **Under-sanitized:** several paths return raw rusqlite/io strings. Examples: `set_host_monitoring_credential`, `clear_metrics_data` and `import_database` use `db::open_connection(...)?` without `sanitize_error`, and scheduled SFTP/SSH errors go into `TaskRunResult.error` and the DB as-is. | The leaks only disclose local paths and SQLite messages to the same local user, so there's no real exposure. The over-sanitizing is a UX/security-usability bug: the lockout policy is invisible. | Pass the lockout/wrong-password messages through for the `vault` unlock context. Wrap the remaining raw `?`s in `sanitize_error` for consistency. | S |
| IPC-013 | Info | `src/components/TerminalComponent.tsx:94-106`; `@xterm/xterm` 6.0.0 `lib/xterm.mjs` (OscLinkProvider / default `Ol` handler) | No `linkHandler` is configured. xterm's built-in OSC 8 handling accepts only `http:`/`https:` URIs (`allowNonHttpProtocols` false), shows `confirm("Do you want to navigate to …")`, then calls `window.open()`. A malicious SSH server can emit clickable hyperlinks whose visible text differs from the target. | The user has to click and confirm, so this is phishing only. What `window.open` does in the Tauri 2.11 webview (blocked, new webview, or external browser) is UNVERIFIED. | Set `linkHandler` to route through a backend or opener call that shows the real URL, or set `linkHandler: { activate: () => {} }` to disable OSC 8. | S |

## Checked and clean

- **HTML-injection sinks.** There are none in app code: 0 hits for `dangerouslySetInnerHTML`/`innerHTML`/`outerHTML`/`insertAdjacentHTML`/`document.write`/`eval(`/`new Function` in non-test `src/`. All remote-sourced data is rendered as React text nodes and escaped: scheduled-task output, SFTP listings, monitoring values, discovered hostnames and error strings.
- **AI output.** It is rendered as plain text (`AIAssistant.tsx:244`, `<div className="whitespace-pre-wrap">{msg.content}</div>`), with no markdown renderer and no HTML. The model has no tool or IPC access, so prompt injection through scanned hostnames placed in the system context (`AIAssistant.tsx:100-115`) can't cause any action. `send_ai_chat` goes through Rust to `Ollama::default()` (127.0.0.1:11434). The model name and messages are passed through unvalidated, which is fine.
- **vis-network labels and titles.** vis-network 10.1.2 `Popup.setText` uses `this.frame.innerText` for string titles (`peer/esm/vis-network.js:13120-13131`, with a comment citing XSS). `NetworkTopologyView.tsx:73,85` passes strings, and labels are drawn on canvas. mDNS or scanned hostnames can't inject HTML.
- **xterm escape handling.**
  - Only `@xterm/addon-fit` is loaded (`TerminalComponent.tsx:3,105-106`). There is no clipboard addon, so OSC 52 isn't implemented (no OSC 52 handler in `xterm.mjs`), and no web-links addon.
  - No `onTitleChange` or custom `parser.register*` handlers exist.
  - The "Clipboard sync" toolbar toggle (`TerminalComponent.tsx:45,252`) is a UI-only state flag that doesn't connect to anything.
- **Command/argument injection in `launch_ssh_external` / `connect_rdp`.** Both validate the address as a strict IPv4 address or RFC 1123 hostname (`validation.rs:24-71`) and the username with `^[a-zA-Z0-9_\-\.]+$` (`validation.rs:84`). `launcher.rs` passes arguments as an argv array with `--` before the target, both for `cmd /C start ssh -- target` and on Linux. None of those characters are cmd metacharacters.
- **SQL injection.** Every query that takes webview input is parameterized: `search_credentials` (`credentials.rs:632-642`), `search_hosts` (`host_tracker.rs:251-261`), `get_audit_logs`/`count` (`audit.rs:49-85`, including LIMIT/OFFSET), `remove_saved_hosts` (placeholders), and `get_metrics_range` (`monitoring.rs:714-726`). `export_database` uses the backup API rather than `VACUUM INTO` string formatting (`lib.rs:1712-1722`). `has_required_columns` formats only constant table names.
- **Vault gating of secrets.** Every command that decrypts goes through `credential_access()`: `connect_ssh`, `start_ssh_tunnel`, `get_remote_hosts_health`, `add_credential`, `get_credential`, `reveal_credential_password`, `update_credential`, and scheduler `resolve_cred_for_task` (`scheduler.rs:373`). `delete_credential` takes `credential_gate()` (`lib.rs:1378`). EDW-15's gate is used at every IPC call site. The lock-order and rekey correctness belong to Phase 2A.
- **Secret-free views.** `get_credential` returns `CredentialFrontendView` with no secrets. `list_credentials`/`search_credentials` return `CredentialSummary` (`credentials.rs:30-40`: name, username, type, host, port, timestamps), which is plaintext metadata already stored unencrypted in SQLite, so it isn't vault-gated. That's acceptable.
- **Key-file reads through `key_path`.** A credential's `key_path` can point at any file, and `ssh_auth.rs:17-21` reads it. The content is never returned: parse errors become "SSH key authentication failed" (`ssh_auth.rs:33-36`), and publickey auth doesn't transmit the key. Only an io error string (for example "No such file") can reach the terminal.
- **`initialize_vault`.** It refuses to re-initialize (`vault.rs:139-145`). `unlock_vault` has persisted lockout (see CLAUDE.md item 10, not re-audited here).
- **`scan_network`.** CIDR is regex-validated (`lib.rs:429`) and capped at /16 (`scanner.rs:84`). The claim happens before spawn (`lib.rs:442`).
- **`clear_metrics_data`.** It deletes only `metrics_history`/`alert_history` (`lib.rs:1694-1697`), with fixed SQL and no input. The worst case is losing history.
- **Tunnels.** The local listener binds `127.0.0.1` only (`ssh_tunnel.rs:185`). Host/port inputs are validated (`lib.rs:275-283`), and the tunnel uses the interactive host-key prompt (`ssh_tunnel.rs:17`, `ssh::Client`).
- **Non-interactive host-key policy.** SFTP (`sftp.rs:47-57`) and one-shot exec (`ssh_exec.rs:43-55`) reject any key that isn't already trusted. The only accept-all handler is `BenchHandler` inside `#[cfg(test)]` (`ssh_pool.rs:370-377`).
- **Events.**
  - All emits use `app.emit` (a broadcast). Sensitive payloads are `ssh_data_{id}` (terminal output), `ssh-host-key-verification` (host, fingerprint, key bytes) and `ai-chat-response`.
  - There is exactly one webview, `main`: no `WebviewWindowBuilder`, `window.open` or `<iframe>` in `src/` or `src-tauri/src`, and the capability is window-scoped with no `remote` URLs. No other window or origin can listen.
  - The Rust side registers no event listeners (0 `.listen` in `src-tauri/src`), so frontend `emit` can't drive backend behaviour. It can only spoof frontend listeners (IPC-008).
  - Hardening is optional: `emit_to("main", …)`.
- **Updater.** It uses a minisign pubkey plus an HTTPS endpoint (`tauri.conf.json`). `updater:default` lets the webview trigger install, but only of signed artifacts.
- **CSP `script-src 'self'`.** It holds: `index.html` has no inline scripts and no `unsafe-eval`. `style-src 'unsafe-inline'` is needed by React inline styles and xterm, and is low risk without script injection.

## UNVERIFIED / needs follow-up

- Whether Tauri 2.11.5 adds `ipc:`/`http://ipc.localhost` to `connect-src` automatically when the localhost wildcards are removed (IPC-007). Verify in a built app before tightening.
- What `window.open()` does in the Tauri 2.11 webview, which is the last step of xterm's default OSC 8 handler (IPC-013).
- Whether a `rusqlite` backup onto an existing *non*-SQLite file fails without truncating it (IPC-009 assumes it errors with `SQLITE_NOTADB` before writing). This needs a quick test.
- The correctness of the `credential_gate` rekey serialization (EDW-15) belongs to Phase 2A. This phase only confirmed that every IPC call site uses `credential_access()`/`credential_gate()`.
- The CLAUDE.md "Commands" section and IPC-001's premise ("SFTP password-only"): SFTP takes the password in cleartext from the webview (`lib.rs:1546,1577`), which is documented design. Moving SFTP to `credential_id` lookup, like SSH, would remove the need for `reveal_credential_password` in `RemoteManager.tsx`. That's a design follow-up, not a verified defect.

## Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 0 |
| Medium | 4 (IPC-001, IPC-002, IPC-003, IPC-004) |
| Low | 7 (IPC-005 to IPC-011) |
| Info | 2 (IPC-012, IPC-013) |

## Appendix: per-command table (73)

"Vault-gated" means the command fails when the vault is locked. "Abuse" is what a compromised webview gains; "none" means read-only local state or harmless. Lines are the `fn` line in the current tree.

| Command | Validation | Vault-gated? | Webview-abuse potential | Notes |
|---|---|---|---|---|
| `connect_ssh` (lib.rs:207) | host/port/user validated in `ssh.rs:233-238`; `id` free-form | Only when `credential_id` is set | Uses any credential against any host (IPC-003) | Errors are verbatim by design (CLAUDE.md item 7) |
| `write_ssh` (ssh.rs:519) | Session id lookup | No | Types into the user's open sessions | Needs a live session id, which the webview knows |
| `resize_ssh` (ssh.rs:546) | None needed | No | None | |
| `disconnect_ssh` (ssh.rs:570) | None needed | No | DoS own sessions | |
| `start_ssh_tunnel` (lib.rs:260) | Hosts and ports validated | With credential | Opens a 127.0.0.1 listener into remote networks; reachable from the webview via CSP (IPC-007) | |
| `list_ssh_tunnels` (lib.rs:328) | n/a | No | None | |
| `close_ssh_tunnel` (lib.rs:335) | n/a | No | None | |
| `launch_ssh_external` (lib.rs:347) | IP/hostname + username regex; `--` | No | Spawns a terminal; **never invoked** (IPC-010) | Injection-safe |
| `connect_rdp` (lib.rs:358) | IP/hostname | No | Spawns `mstsc` (Windows) | Injection-safe |
| `get_tailscale_status` (lib.rs:368) | No input | No | None | Fixed argv (`tailscale status --json`) |
| `start_discovery` (lib.rs:375) | No input | No | None | Singleton |
| `check_ai_status` (lib.rs:380) | No input | No | None | |
| `list_ai_models` (lib.rs:385) | No input | No | None | |
| `send_ai_chat` (lib.rs:399) | Role mapped to an enum; model/content free | No | Local LLM use only | Output rendered as text |
| `scan_network` (lib.rs:423) | CIDR regex, ≤ /16 | No | LAN ping/port scan | By design |
| `stop_scan` (lib.rs:476) | n/a | No | None | |
| `get_scan_progress` (lib.rs:481) | n/a | No | None | |
| `is_scanning` (lib.rs:486) | n/a | No | None | |
| `get_discovered_hosts` (lib.rs:491) | `limit` unbounded | No | None | Parameterized |
| `get_host_details` (lib.rs:501) | IPv4 | No | None; **never invoked** | |
| `search_discovered_hosts` (lib.rs:512) | Free text, parameterized | No | None; **never invoked** | |
| `delete_discovered_host` (lib.rs:522) | IPv4 | No | Data deletion | |
| `get_saved_hosts` (lib.rs:738) | n/a | No | None | |
| `upsert_saved_host` (lib.rs:743) | Port only; name/address/protocol unvalidated | No | Can repoint a saved host name at an attacker address | Address is validated later at connect time |
| `update_saved_host` (lib.rs:764) | Port only | No | Same as above | |
| `remove_saved_hosts` (lib.rs:787) | Parameterized | No | Data deletion | |
| `list_scheduled_tasks` (lib.rs:809) | n/a | No | Reads task commands/output | |
| `get_scheduled_task` (lib.rs:815) | n/a | No | None; **never invoked** | |
| `add_scheduled_task` (lib.rs:824) | Cron parsed; paths `validate_path` only for SFTP types | No (runs need unlock if a credential is set) | Persistent remote RCE + local file R/W (IPC-001, IPC-011) | No audit |
| `update_scheduled_task` (lib.rs:864) | Same | No | Same | |
| `remove_scheduled_task` (lib.rs:906) | n/a | No | Deletion | |
| `run_scheduled_task_now` (lib.rs:912) | Task id | Unlock needed if a credential is set | Immediate remote exec / file R/W | Errors sanitized; `TaskRunResult.error` raw |
| `get_remote_hosts_health` (lib.rs:990) | n/a | Soft (metrics only when unlocked) | Triggers credentialed logins to bound hosts (IPC-003) | |
| `set_host_monitoring_credential` (lib.rs:1030) | None (no existence/type check) | No | Binds any credential to any host (IPC-003) | Raw `open_connection` error |
| `preflight_check` (lib.rs:1063) | IP/hostname | No | Ping/DNS probe | |
| `check_host_health` (lib.rs:1071) | Host/port/user | No | SSH login probe with a supplied password | |
| `get_system_metrics` (lib.rs:1098) | n/a | No | Local system info | |
| `get_metrics_history` (lib.rs:1111) | None | No | None; **never invoked** | |
| `get_alert_history` (lib.rs:1135) | None | No | None; **never invoked** | |
| `add_alert_rule` (lib.rs:1158) | Serde enums; threshold/cooldown unbounded | No | Alert spam | In-memory |
| `remove_alert_rule` (lib.rs:1163) | n/a | No | None | |
| `get_alert_rules` (lib.rs:1168) | n/a | No | None | |
| `is_vault_initialized` (lib.rs:1174) | n/a | No | None | |
| `initialize_vault` (lib.rs:1182) | ≥ 12 chars; refuses re-init | n/a | None | |
| `unlock_vault` (lib.rs:1195) | Non-empty; persisted lockout | n/a | Brute force is rate-limited | Lockout message hidden (IPC-012) |
| `lock_vault` (lib.rs:1210) | n/a | n/a | DoS only | |
| `is_vault_locked` (lib.rs:1218) | n/a | No | None | |
| `get_vault_settings` (lib.rs:1223) | n/a | No | None | |
| `update_vault_settings` (lib.rs:1230) | **None** | **No** | Disable auto-lock; overflow (IPC-006) | Security toggle not enforced (IPC-004) |
| `change_master_password` (lib.rs:1500) | Current non-empty, new ≥ 12 | Requires the current password | None beyond knowing it | Gate: EDW-15 |
| `add_credential` (lib.rs:1242) | Name, username, host, port | Yes | Store arbitrary `key_path` | |
| `get_credential` (lib.rs:1288) | Id | Yes | Metadata only | No secrets (verified) |
| `reveal_credential_password` (lib.rs:1304) | Id | Yes | **Plaintext password**; no step-up (IPC-005) | Audited as generic access |
| `list_credentials` (lib.rs:1320) | n/a | No | Credential names/usernames/hosts | Plaintext metadata |
| `update_credential` (lib.rs:1329) | Name, username only (host/port **not** validated, unlike add) | Yes | Overwrite credentials | |
| `delete_credential` (lib.rs:1373) | Id | Gate held; key not needed, **works while locked** | Deletes credentials while locked | Audited |
| `search_credentials` (lib.rs:1385) | Parameterized | No | Metadata | |
| `verify_ssh_host_key` (lib.rs:1395) | Host/port | No | Oracle on known hosts | |
| `trust_ssh_host_key` (lib.rs:1413) | Host/port only; fingerprint/key free | No | **Pins any key for any host** (IPC-002) | No audit |
| `respond_ssh_host_key_verification` (lib.rs:1447) | request_id | No | Accepts any pending prompt | By design |
| `get_known_ssh_hosts` (lib.rs:1456) | n/a | No | None | |
| `remove_ssh_host_key` (lib.rs:1466) | Host/port | No | Forces a re-prompt | |
| `update_ssh_host_trust` (lib.rs:1482) | Host/port; enum | No | Flips a stored key to trusted (IPC-002) | No audit |
| `get_audit_logs` (lib.rs:1520) | Parameterized; limit unbounded | No | Reads the audit log | |
| `get_audit_log_count` (lib.rs:1530) | Same | No | None; **never invoked** | |
| `sftp_upload_file` (lib.rs:1541) | Host/port/user; `validate_path` only | No (password passed in clear) | **Arbitrary local file read → exfiltration** (IPC-001) | Host key must be trusted |
| `sftp_download_file` (lib.rs:1572) | Same | No | **Arbitrary local file write** (IPC-001) | Same |
| `sftp_list_directory` (lib.rs:1603) | Host/port/user; remote path free | No | Remote listing | |
| `sftp_remote_exists` (lib.rs:1622) | Host/port/user | No | Remote oracle; **never invoked** | |
| `get_app_info` (lib.rs:1652) | n/a | No | Discloses the app data path | Intended (Settings page) |
| `clear_metrics_data` (lib.rs:1684) | n/a | No | Deletes metric/alert history | Fixed SQL |
| `export_database` (lib.rs:1702) | `validate_path` | No | Writes the DB (Argon2 hash + ciphertext) anywhere; overwrites other SQLite files (IPC-009) | |
| `import_database` (lib.rs:1727) | `validate_path`; Quasar signature check | **No** | Replaces all state: trust, tasks, vault (IPC-009) | Raw errors |
