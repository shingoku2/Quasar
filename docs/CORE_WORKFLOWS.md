# Quasar Core Workflows

This document describes the main user workflows for remote management, automation, and security. Checked against the UI on 2026-09-25.

---

## 1. Vault and credentials

- **First launch**: On startup the app asks for a new master password (the vault init dialog) until one is set. Setting it leaves the vault unlocked. On later launches it asks to unlock.
- **Unlock**: After a restart, auto-lock or database import, enter the master password to unlock. Five, ten and fifteen failed attempts lock unlocking for 5, 15 and 60 minutes (persisted across restarts).
- **Vault settings**: Settings → Security: auto-lock timeout (1-1440 minutes), lock now, change master password (refused, naming each credential, if some can't be decrypted).
- **Credentials**: Add SSH (password or SSH key), RDP, database, API or other credentials in Security → Credentials. Encrypted at rest (AES-256-GCM). Setting a credential's **host** binds it: it then only works against that host (moving or clearing the binding later asks for a native OS confirmation). Revealing a stored password also asks for a native confirmation and is audited; the edit form never shows the stored password (leave it blank to keep it).
- **Known hosts and audit log**: Security → Known Hosts (pinned SSH host keys: trust, reject, remove) and Security → Audit Log.
- **SSH host keys**: The first interactive connection to a host shows its key fingerprint; trust it once or permanently. A *changed* key shows a MITM warning and needs an explicit "I verified" plus a native OS confirmation. Scheduled tasks, monitoring polls and SFTP never prompt: they only accept a key already trusted for that host and port.

---

## 2. Remote hosts and SSH

- **Add host**: Remote → Add/save a host (address, port, username, protocol). Optionally link a credential.
- **Connect**: From Remote or Dashboard, select a host and connect. Use vault credential or enter password. SSH terminal opens (xterm.js).
- **Multiple sessions**: Open several terminals to different hosts; each has its own session. Commands run without blocking other sessions (flow control uses `Channel::wait()`).
- **Quick connect**: The Dashboard's Quick Connect widget lists saved hosts for one-click SSH.

---

## 3. Scheduled tasks (Automation)

- **Location**: Automation (sidebar).
- **Task types**: Choose **SSH command**, **Upload file (SFTP)**, or **Download file (SFTP)**.
  - **SSH command**: Enter the command to run on the host (e.g. `/opt/backup.sh`).
  - **Upload file**: Pick the local file with the file dialog (paths can't be typed; the backend only accepts a path chosen in its own dialog) and enter the remote path (e.g. `/home/user/file.zip`).
  - **Download file**: Enter the remote path and pick the save location with the dialog. File transfers use a vault password credential for SFTP and time out after an hour.
- **Add task**: Name, **cron schedule**, host, optional credential (a host-bound credential must match the task's host). Enabled by default. Creating, changing, deleting and running tasks are audited.
- **Cron format**: **6 fields** (sec min hour day month dow), e.g. `0 0 9 * * *` = daily at 9:00. Use the placeholder in the form as a guide.
- **Execution**: The scheduler checks every 60 seconds and starts due tasks in the background (a slow task doesn't delay others). A task never runs twice at once. Password or SSH key from the vault. If the vault is locked, tasks that need a credential are skipped. A command's exit status is recorded correctly (non-zero = Failed); output is capped at 1 MiB.
- **Last run**: Each task shows last run time, status (Success / Failed), and optional error or output snippet.
- **Run now**: Use the play button to run a task once and see the full result (output/error) in the result panel. It's refused while the same task is already running.

---

## 4. File transfer (SFTP)

- **From Remote**: The SFTP button on a saved host opens an `SFTP: <host>` session tab to browse it, and upload or download files. Local files are chosen with the native file dialog; each choice is good for one transfer (a failed transfer, e.g. a wrong password, keeps it for a retry).
- **Tauri commands**: `sftp_list_directory`, `sftp_upload_file`, `sftp_download_file`.
- **Authentication**: SFTP currently requires **password-based** credentials. SSH key credentials are not yet supported for SFTP; the credential selector and scheduled-task UI hide or filter key-based credentials for SFTP flows.

---

## 5. Monitoring and health

- **Dashboard**: Real-time metrics (CPU, memory, disks), Hosts Online count (reachable saved hosts), Vault Auto-lock status.
- **Monitoring view**: Alert Rules panel (create/edit/remove rules; saved in the database and kept across restarts); remote host health (ping + optional SSH metrics). Assign credentials per host for SSH-based checks.
- **Health before connect**: Use preflight/health check to validate a host before opening a terminal.
- **Note (Linux only)**: SSH-based remote metric collection (load average, process count via SSH) is currently only implemented for Linux targets in `monitoring.rs`. Windows and macOS remote hosts return ping-only health data.
- **Alert recovery**: When a metric returns below its threshold after an alert fires, an `alerts-recovered` event is emitted and shown in the Recent Activity feed.

---

## 6. Network discovery

- **Dashboard**: Use the Network Topology card; switch between List and Topology. Scan a CIDR to discover hosts and services.
- **Discovered hosts**: View details, save to Remote hosts, connect via SSH, or remove from discovered list.

---

## 7. Connect via Tailscale

- **Requirement**: The `tailscale` CLI must be installed and logged in on this machine. Quasar only shells out to `tailscale status --json` locally — no API key or account setup inside the app.
- **Location**: Remote → Inventory, in the "Tailscale" panel beside LAN Discovery. Shows your tailnet peers (not your own node), online status, OS, and address.
- **Add a peer**: Click **Add** on a peer to open the Add Host dialog prefilled for it, then review and save it as an SSH host. The stored address is the peer's MagicDNS name (e.g. `myhost.tailXXXX.ts.net`) when MagicDNS is enabled for the tailnet, otherwise its `100.x.y.z` address. A peer that matches an already-saved host shows **Saved** instead.
- **Badge on saved hosts**: Any saved host whose address matches a tailnet peer shows a Tailscale badge with an online/offline dot in the host list's Address column.
- **Connecting**: Peers with an "SSH" chip run Tailscale SSH. Connecting to one lets you leave the password blank in the manual credential prompt — Quasar authenticates the SSH session by tailnet identity (`none` auth) instead. Peers without the chip need a normal password or SSH key as usual.
- **Not supported**: Tailscale SSH "check mode" (the browser re-verification prompt some tailnet policies require) is not implemented; a connection that requires it will show an authentication error in the terminal.

---

## 8. Appearance

- **Settings → Appearance**: Choose **Dark** or **Light** theme; both use readable text (light theme overrides Tailwind text classes so labels and headers are dark on light backgrounds). Set accent color; changes apply immediately. Terminal theme is independent (uses its own theme prop, e.g. default or Solarized Light).

---

## Quick reference: cron (scheduled tasks)

Format: **sec min hour day month day_of_week** (6 fields).

| Example           | Meaning              |
|-------------------|----------------------|
| `0 0 9 * * *`     | Daily at 9:00       |
| `0 30 8 * * 1-5`  | 8:30 on weekdays     |
| `0 0 0 1 * *`     | First of month 00:00 |
| `0 */15 * * * *`  | Every 15 minutes     |
