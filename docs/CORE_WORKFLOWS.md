# Quasar Core Workflows

This document describes the main user workflows for remote management, automation, and security.

---

## 1. Vault and credentials

- **First launch**: Initialize the vault with a strong master password (Security → Vault).
- **Unlock**: After restart or auto-lock, enter the master password to unlock.
- **Credentials**: Add SSH (password or SSH key), RDP, database, or API credentials in Security → Credentials. Associate with hosts if desired. Credentials are encrypted at rest (AES-256-GCM).
- **SSH host keys**: On first SSH connection to a host, verify and trust the host key to avoid MITM prompts later.

---

## 2. Remote hosts and SSH

- **Add host**: Remote → Add/save a host (address, port, username, protocol). Optionally link a credential.
- **Connect**: From Remote or Dashboard, select a host and connect. Use vault credential or enter password. SSH terminal opens (xterm.js).
- **Multiple sessions**: Open several terminals to different hosts; each has its own session. Commands run without blocking other sessions (flow control uses `Channel::wait()`).
- **Quick connect**: From Dashboard Network Topology or host list, use quick connect for one-click SSH.

---

## 3. Scheduled tasks (Automation)

- **Location**: Automation (sidebar).
- **Task types**: Choose **SSH command**, **Upload file (SFTP)**, or **Download file (SFTP)**.
  - **SSH command**: Enter the command to run on the host (e.g. `/opt/backup.sh`).
  - **Upload file**: Enter local path (e.g. `C:\backup\file.zip`) and remote path (e.g. `/home/user/file.zip`).
  - **Download file**: Enter remote path and local path. File transfer tasks use the same vault credential (password) for SFTP.
- **Add task**: Name, **cron schedule**, host, optional credential. Enabled by default.
- **Cron format**: **6 fields** (sec min hour day month dow), e.g. `0 0 9 * * *` = daily at 9:00. Use the placeholder in the form as a guide.
- **Execution**: Background scheduler checks every 60 seconds; due tasks run via SSH (command) or SFTP (upload/download). Password or SSH key from vault. If the vault is locked, tasks that need a credential are skipped.
- **Last run**: Each task shows last run time, status (Success / Failed), and optional error or output snippet.
- **Run now**: Use the play button to run a task once and see the full result (output/error) in the result panel.

---

## 4. File transfer (SFTP)

- **From SSH session**: Use the SFTP/file UI in the session to browse, upload, and download files.
- **Tauri commands**: `sftp_upload_file`, `sftp_download_file`, `sftp_list_directory`, `sftp_remote_exists` are available for workflows or future UI.

---

## 5. Monitoring and health

- **Dashboard**: Real-time metrics (CPU, memory, disks), Hosts Online count (reachable saved hosts), Vault Auto-lock status.
- **Monitoring view**: Configure alert rules; view remote host health (ping + optional SSH metrics). Assign credentials per host for SSH-based checks.
- **Health before connect**: Use preflight/health check to validate a host before opening a terminal.

---

## 6. Network discovery

- **Dashboard**: Use the Network Topology card; switch between List and Topology. Scan a CIDR to discover hosts and services.
- **Discovered hosts**: View details, save to Remote hosts, connect via SSH, or remove from discovered list.

---

## Quick reference: cron (scheduled tasks)

Format: **sec min hour day month day_of_week** (6 fields).

| Example           | Meaning              |
|-------------------|----------------------|
| `0 0 9 * * *`     | Daily at 9:00       |
| `0 30 8 * * 1-5`  | 8:30 on weekdays     |
| `0 0 0 1 * *`     | First of month 00:00 |
| `0 */15 * * * *`  | Every 15 minutes     |
