//! Tailscale integration via the local `tailscale` CLI.
//!
//! Quasar never talks to the Tailscale control plane or API directly; it only
//! shells out to `tailscale status --json` on the local machine and normalises
//! the result for the frontend. The command arguments are fixed — no user input
//! is ever passed to the CLI — and every address emitted to the frontend is run
//! through the same `validate_hostname` / `validate_ip` checks the rest of the
//! app applies to user input, so nothing the CLI returns can bypass them
//! downstream.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::validation::{validate_hostname, validate_ip};

/// How long to wait for `tailscale status --json` before giving up.
const STATUS_TIMEOUT: Duration = Duration::from_secs(5);

/// One peer (or the local node) on the tailnet, as shown to the frontend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TailscalePeer {
    /// Stable node identifier (the `ID` field from `tailscale status`).
    pub id: String,
    pub hostname: String,
    /// MagicDNS name with the trailing dot stripped; empty when not available.
    pub dns_name: String,
    /// The node's 100.x.y.z tailnet IPv4 address, if it has one.
    pub ipv4: Option<String>,
    pub os: String,
    pub online: bool,
    /// True when the node advertises SSH host keys, i.e. Tailscale SSH is enabled
    /// on it and it will accept identity-based (`none`) authentication.
    pub tailscale_ssh: bool,
    pub exit_node: bool,
    /// The address Quasar should store for this peer: the MagicDNS name when
    /// MagicDNS is enabled for the tailnet, otherwise the IPv4 address.
    pub preferred_address: String,
}

/// Result of `get_tailscale_status`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TailscaleStatus {
    /// False when no `tailscale` binary could be found; every other field is
    /// then empty/default and the UI should show an install hint.
    pub installed: bool,
    /// `BackendState` as reported by the CLI, e.g. `Running`, `NeedsLogin`, `Stopped`.
    pub backend_state: String,
    pub magic_dns_enabled: bool,
    pub magic_dns_suffix: Option<String>,
    pub self_node: Option<TailscalePeer>,
    /// Peers (excluding the local node), sorted online-first, then by hostname.
    pub peers: Vec<TailscalePeer>,
}

impl TailscaleStatus {
    fn not_installed() -> Self {
        Self {
            installed: false,
            backend_state: String::new(),
            magic_dns_enabled: false,
            magic_dns_suffix: None,
            self_node: None,
            peers: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Raw CLI JSON shapes — only the fields we actually use. `#[serde(default)]`
// keeps parsing tolerant of fields that are missing or added in newer CLIs.
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawStatus {
    #[serde(rename = "BackendState")]
    backend_state: String,
    #[serde(rename = "MagicDNSSuffix")]
    magic_dns_suffix: String,
    #[serde(rename = "CurrentTailnet")]
    current_tailnet: Option<RawTailnet>,
    #[serde(rename = "Self")]
    self_node: Option<RawPeer>,
    #[serde(rename = "Peer")]
    peer: Option<HashMap<String, RawPeer>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawTailnet {
    #[serde(rename = "MagicDNSEnabled")]
    magic_dns_enabled: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawPeer {
    #[serde(rename = "ID")]
    id: String,
    #[serde(rename = "HostName")]
    host_name: String,
    #[serde(rename = "DNSName")]
    dns_name: String,
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Vec<String>,
    #[serde(rename = "OS")]
    os: String,
    #[serde(rename = "Online")]
    online: bool,
    #[serde(rename = "ExitNode")]
    exit_node: bool,
    /// Present (and non-empty) only when Tailscale SSH is enabled on the node.
    #[serde(rename = "sshHostKeys")]
    ssh_host_keys: Option<Vec<String>>,
}

/// Locate the `tailscale` CLI: `tailscale` on PATH first, then the well-known
/// per-platform install locations.
pub fn find_tailscale_binary() -> Option<PathBuf> {
    if let Some(path) = find_on_path("tailscale") {
        return Some(path);
    }

    let fallbacks: &[&str] = if cfg!(target_os = "macos") {
        &["/Applications/Tailscale.app/Contents/MacOS/Tailscale"]
    } else if cfg!(target_os = "windows") {
        &["C:\\Program Files\\Tailscale\\tailscale.exe"]
    } else {
        &["/usr/bin/tailscale"]
    };

    fallbacks
        .iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    let names: Vec<String> = if cfg!(target_os = "windows") {
        vec![format!("{}.exe", name), name.to_string()]
    } else {
        vec![name.to_string()]
    };
    std::env::split_paths(&path_var)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|candidate| candidate.is_file())
}

/// Run `tailscale status --json` and normalise the output.
///
/// A missing binary is not an error — it yields `installed: false` so the UI
/// can show an install hint. A non-zero exit, a timeout, or unparseable output
/// is an `Err`, which the command layer sanitises before it reaches the frontend.
pub async fn fetch_status() -> Result<TailscaleStatus, String> {
    let Some(binary) = find_tailscale_binary() else {
        return Ok(TailscaleStatus::not_installed());
    };

    let mut command = tokio::process::Command::new(&binary);
    command.args(["status", "--json"]);
    command.stdin(std::process::Stdio::null());
    command.kill_on_drop(true);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: don't flash a console window from the GUI app.
        command.creation_flags(0x0800_0000);
    }

    let output = tokio::time::timeout(STATUS_TIMEOUT, command.output())
        .await
        .map_err(|_| format!("tailscale status timed out after {:?}", STATUS_TIMEOUT))?
        .map_err(|e| format!("Failed to run {}: {}", binary.display(), e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // `tailscale status --json` exits non-zero in some states (e.g. the daemon
    // is stopped) while still printing a valid JSON document with a
    // `BackendState`. Prefer the parsed state over the exit code in that case.
    match parse_status(&stdout) {
        Ok(status) => Ok(status),
        Err(parse_err) => {
            if output.status.success() {
                Err(parse_err)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!(
                    "tailscale status exited with {}: {}",
                    output.status,
                    stderr.trim()
                ))
            }
        }
    }
}

/// Pure parser for the `tailscale status --json` document (unit-testable).
pub fn parse_status(json: &str) -> Result<TailscaleStatus, String> {
    let raw: RawStatus = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse tailscale status JSON: {}", e))?;

    let magic_dns_enabled = raw
        .current_tailnet
        .as_ref()
        .map(|t| t.magic_dns_enabled)
        .unwrap_or(false);
    let magic_dns_suffix = {
        let suffix = raw.magic_dns_suffix.trim().trim_end_matches('.');
        if suffix.is_empty() {
            None
        } else {
            Some(suffix.to_string())
        }
    };

    let self_node = raw
        .self_node
        .and_then(|p| normalise_peer(p, magic_dns_enabled));

    let mut peers: Vec<TailscalePeer> = raw
        .peer
        .unwrap_or_default()
        .into_values()
        .filter_map(|p| normalise_peer(p, magic_dns_enabled))
        .collect();

    // Online first, then case-insensitively by hostname, then by id for a
    // deterministic order when hostnames collide.
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(TailscaleStatus {
        installed: true,
        backend_state: raw.backend_state,
        magic_dns_enabled,
        magic_dns_suffix,
        self_node,
        peers,
    })
}

/// Convert a raw CLI peer into the frontend shape, or `None` if the peer has
/// no usable address (IPv6-only, or an address that fails validation).
fn normalise_peer(raw: RawPeer, magic_dns_enabled: bool) -> Option<TailscalePeer> {
    let dns_name = raw.dns_name.trim().trim_end_matches('.').to_string();
    let dns_name = if dns_name.is_empty() || validate_hostname(&dns_name).is_err() {
        String::new()
    } else {
        dns_name
    };

    let ipv4 = raw
        .tailscale_ips
        .iter()
        .map(|ip| ip.trim())
        .find(|ip| validate_ip(ip).is_ok())
        .map(str::to_string);

    let preferred_address = if magic_dns_enabled && !dns_name.is_empty() {
        dns_name.clone()
    } else {
        ipv4.clone()?
    };

    let hostname = if raw.host_name.trim().is_empty() {
        preferred_address.clone()
    } else {
        raw.host_name.trim().to_string()
    };

    let tailscale_ssh = raw
        .ssh_host_keys
        .as_ref()
        .map(|keys| !keys.is_empty())
        .unwrap_or(false);

    Some(TailscalePeer {
        id: raw.id,
        hostname,
        dns_name,
        ipv4,
        os: raw.os,
        online: raw.online,
        tailscale_ssh,
        exit_node: raw.exit_node,
        preferred_address,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Modelled on real `tailscale status --json` output, trimmed to the fields
    /// we consume plus a few we ignore (to check tolerance of extra fields).
    fn fixture(magic_dns_enabled: bool) -> String {
        format!(
            r#"{{
  "Version": "1.80.0-t123",
  "TUN": true,
  "BackendState": "Running",
  "AuthURL": "",
  "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
  "Self": {{
    "ID": "nSELF",
    "PublicKey": "nodekey:self",
    "HostName": "my-laptop",
    "DNSName": "my-laptop.tail1234.ts.net.",
    "OS": "linux",
    "UserID": 1,
    "TailscaleIPs": ["100.64.0.1", "fd7a:115c:a1e0::1"],
    "Online": true,
    "ExitNode": false,
    "Active": false
  }},
  "MagicDNSSuffix": "tail1234.ts.net",
  "CurrentTailnet": {{
    "Name": "example@example.com",
    "MagicDNSSuffix": "tail1234.ts.net",
    "MagicDNSEnabled": {magic_dns_enabled}
  }},
  "Peer": {{
    "nodekey:vps": {{
      "ID": "nVPS",
      "HostName": "vps-a8fa83ff",
      "DNSName": "vps-a8fa83ff.tail1234.ts.net.",
      "OS": "linux",
      "TailscaleIPs": ["100.64.0.2", "fd7a:115c:a1e0::2"],
      "Online": true,
      "ExitNode": false,
      "sshHostKeys": ["ssh-ed25519 AAAA..."]
    }},
    "nodekey:phone": {{
      "ID": "nPHONE",
      "HostName": "Pixel",
      "DNSName": "pixel.tail1234.ts.net.",
      "OS": "android",
      "TailscaleIPs": ["100.64.0.3"],
      "Online": false,
      "ExitNode": false
    }},
    "nodekey:v6only": {{
      "ID": "nV6",
      "HostName": "ipv6-only",
      "DNSName": "",
      "OS": "linux",
      "TailscaleIPs": ["fd7a:115c:a1e0::9"],
      "Online": true,
      "ExitNode": false
    }},
    "nodekey:exit": {{
      "ID": "nEXIT",
      "HostName": "exit-box",
      "DNSName": "exit-box.tail1234.ts.net.",
      "OS": "linux",
      "TailscaleIPs": ["100.64.0.4"],
      "Online": true,
      "ExitNode": true,
      "sshHostKeys": []
    }}
  }}
}}"#
        )
    }

    fn peer<'a>(status: &'a TailscaleStatus, hostname: &str) -> &'a TailscalePeer {
        status
            .peers
            .iter()
            .find(|p| p.hostname == hostname)
            .unwrap_or_else(|| panic!("peer {} missing", hostname))
    }

    #[test]
    fn parses_self_and_peers_with_magic_dns() {
        let status = parse_status(&fixture(true)).unwrap();
        assert!(status.installed);
        assert_eq!(status.backend_state, "Running");
        assert!(status.magic_dns_enabled);
        assert_eq!(status.magic_dns_suffix.as_deref(), Some("tail1234.ts.net"));

        let me = status.self_node.as_ref().expect("self node");
        assert_eq!(me.hostname, "my-laptop");
        // Trailing dot stripped.
        assert_eq!(me.dns_name, "my-laptop.tail1234.ts.net");
        assert_eq!(me.ipv4.as_deref(), Some("100.64.0.1"));
        assert_eq!(me.preferred_address, "my-laptop.tail1234.ts.net");

        // Self is never in `peers`; the IPv6-only node is dropped.
        assert_eq!(status.peers.len(), 3);
        assert!(status.peers.iter().all(|p| p.id != "nSELF"));
        assert!(status.peers.iter().all(|p| p.hostname != "ipv6-only"));

        let vps = peer(&status, "vps-a8fa83ff");
        assert_eq!(vps.preferred_address, "vps-a8fa83ff.tail1234.ts.net");
        assert_eq!(vps.ipv4.as_deref(), Some("100.64.0.2"));
        assert!(vps.online);
        assert!(vps.tailscale_ssh);
        assert!(!vps.exit_node);
        assert_eq!(vps.os, "linux");
    }

    #[test]
    fn prefers_ipv4_when_magic_dns_disabled() {
        let status = parse_status(&fixture(false)).unwrap();
        assert!(!status.magic_dns_enabled);
        let vps = peer(&status, "vps-a8fa83ff");
        assert_eq!(vps.preferred_address, "100.64.0.2");
        // The DNS name is still surfaced for matching/display.
        assert_eq!(vps.dns_name, "vps-a8fa83ff.tail1234.ts.net");
    }

    #[test]
    fn empty_ssh_host_keys_means_no_tailscale_ssh() {
        let status = parse_status(&fixture(true)).unwrap();
        let exit = peer(&status, "exit-box");
        assert!(!exit.tailscale_ssh);
        assert!(exit.exit_node);
        let phone = peer(&status, "Pixel");
        assert!(!phone.tailscale_ssh);
    }

    #[test]
    fn peers_sorted_online_first_then_hostname() {
        let status = parse_status(&fixture(true)).unwrap();
        let order: Vec<&str> = status.peers.iter().map(|p| p.hostname.as_str()).collect();
        // exit-box and vps-a8fa83ff are online (alphabetical), Pixel is offline.
        assert_eq!(order, vec!["exit-box", "vps-a8fa83ff", "Pixel"]);
    }

    #[test]
    fn backend_state_passthrough_for_needs_login_and_stopped() {
        for state in ["NeedsLogin", "Stopped"] {
            let json = format!(
                r#"{{"BackendState":"{}","MagicDNSSuffix":"","Self":null,"Peer":null}}"#,
                state
            );
            let status = parse_status(&json).unwrap();
            assert!(status.installed);
            assert_eq!(status.backend_state, state);
            assert!(status.peers.is_empty());
            assert!(status.self_node.is_none());
            assert!(status.magic_dns_suffix.is_none());
        }
    }

    #[test]
    fn peer_with_invalid_address_is_dropped() {
        let json = r#"{
          "BackendState": "Running",
          "MagicDNSSuffix": "tail1234.ts.net",
          "CurrentTailnet": {"MagicDNSEnabled": true},
          "Peer": {
            "nodekey:bad": {
              "ID": "nBAD",
              "HostName": "bad host",
              "DNSName": "bad_host!.tail1234.ts.net.",
              "OS": "linux",
              "TailscaleIPs": ["999.1.1.1", "not-an-ip"],
              "Online": true
            },
            "nodekey:baddns": {
              "ID": "nBADDNS",
              "HostName": "baddns",
              "DNSName": "bad_dns.tail1234.ts.net.",
              "OS": "linux",
              "TailscaleIPs": ["100.64.0.7"],
              "Online": true
            }
          }
        }"#;
        let status = parse_status(json).unwrap();
        // The peer with only invalid addresses is gone entirely...
        assert!(status.peers.iter().all(|p| p.id != "nBAD"));
        // ...while a peer with an invalid DNS name but a valid IPv4 falls back to the IP.
        let baddns = peer(&status, "baddns");
        assert_eq!(baddns.dns_name, "");
        assert_eq!(baddns.preferred_address, "100.64.0.7");
    }

    #[test]
    fn falls_back_to_address_when_hostname_missing() {
        let json = r#"{
          "BackendState": "Running",
          "Peer": {
            "nodekey:x": {"ID": "nX", "HostName": "", "TailscaleIPs": ["100.64.0.8"], "Online": true}
          }
        }"#;
        let status = parse_status(json).unwrap();
        assert_eq!(status.peers[0].hostname, "100.64.0.8");
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(parse_status("not json").is_err());
        assert!(parse_status("").is_err());
    }
}
