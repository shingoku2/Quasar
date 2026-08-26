# Security Policy

Quasar stores and uses sensitive material — SSH credentials, private keys, and passwords
for the remote infrastructure it manages — so vulnerabilities here can have outsized impact.
We take reports seriously and will work with you to understand and fix confirmed issues.

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.** Publicly
disclosing a vulnerability before a fix is available puts every user's credential vault
and connected infrastructure at risk.

Instead, report privately through GitHub's Security Advisories:

1. Go to the [Security tab](https://github.com/shingoku2/Quasar/security) of this
   repository.
2. Click **"Report a vulnerability"** to open a private draft advisory.
3. Include as much detail as you can — see "What to include" below.

If you're unable to use GitHub's private reporting (e.g. you don't have a GitHub account),
open a regular issue asking a maintainer to provide an alternate contact, without including
any vulnerability details in that issue itself.

### What to include

- A description of the vulnerability and its potential impact.
- Steps to reproduce, or a proof-of-concept (a failing test against this codebase is ideal).
- The affected version/commit.
- Anything you've already ruled out or workarounds you're aware of.

### What to expect

- **Acknowledgment** within 5 business days.
- We'll confirm the issue, assess severity, and keep you updated as a fix is developed.
- Once a fix is released, we'll credit you in the advisory (unless you'd prefer to stay
  anonymous) and coordinate a disclosure timeline with you.
- We don't currently run a paid bug bounty program.

## Scope

In scope:
- The Rust backend (`src-tauri/`) — vault/credential encryption, SSH/SFTP handling, input
  validation, the scheduler, network scanning/discovery.
- The React frontend (`src/`) — especially anywhere it handles credentials, IPC payloads, or
  renders untrusted data (host names, scan results, SSH output).
- The release pipeline (`.github/workflows/`) and update mechanism (`tauri-plugin-updater`
  config) — e.g. anything that could let an unsigned or tampered build reach a user.

Out of scope:
- Vulnerabilities that require an attacker to already have unlocked-vault access or
  arbitrary code execution on the user's machine (game over at that point regardless of
  this app).
- Denial of service against your own local instance (e.g. crashing your own client).
- Findings from purely automated scanners without a demonstrated, working exploit against
  this codebase.

## Supported Versions

Quasar is pre-1.0 and does not yet maintain parallel release branches. Security fixes are
made against the latest release; please upgrade to the newest version before reporting to
confirm the issue is still present. Once 1.0 ships, this section will define which major
versions receive backported fixes.

## Known Design Notes

A few things that look like vulnerabilities at first glance but are intentional — documented
here so reports about them can skip straight to "is the intentional handling actually safe":

- **`connect_ssh` (the interactive terminal path) returns raw connection errors to the
  frontend**, including resolved IPs and which phase (DNS/TCP/handshake) failed, instead of
  a sanitized message. This is deliberate — see `CLAUDE.md`'s Security Notes — so users can
  self-diagnose connection issues. SFTP and scheduled-task SSH paths are sanitized.
- **`list_credentials` works without unlocking the vault.** It only returns metadata (name,
  type, username) — never secret material — so a locked vault still lets the UI show what
  credentials exist. `get_credential` and anything touching secret material requires the
  vault to be unlocked.
