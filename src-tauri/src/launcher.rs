
// `launch_ssh` (external terminal) was removed with its never-invoked IPC command
// `launch_ssh_external` (audit IPC-010); the in-app terminal is the only SSH client.

/// The default RDP port, which `mstsc` uses when the target names none.
#[cfg(any(target_os = "windows", test))]
const RDP_DEFAULT_PORT: u16 = 3389;

/// The single `mstsc` target argument: `/v:host`, or `/v:host:port` for a non-default port.
/// `address` is already validated as an IPv4 address or hostname, so it holds no `:` of its own.
#[cfg(any(target_os = "windows", test))]
fn rdp_target(address: &str, port: Option<u16>) -> String {
    match port {
        Some(port) if port != RDP_DEFAULT_PORT => format!("/v:{address}:{port}"),
        _ => format!("/v:{address}"),
    }
}

#[cfg(target_os = "windows")]
pub fn launch_rdp(address: &str, port: Option<u16>) -> Result<(), String> {
    std::process::Command::new("mstsc")
        .arg(rdp_target(address, port))
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn launch_rdp(_address: &str, _port: Option<u16>) -> Result<(), String> {
    Err("RDP launch only supported on Windows for MVP".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rdp_target_names_a_custom_port() {
        assert_eq!(rdp_target("10.0.0.5", Some(3390)), "/v:10.0.0.5:3390");
        assert_eq!(rdp_target("desk.lan", Some(13389)), "/v:desk.lan:13389");
    }

    #[test]
    fn rdp_target_leaves_the_default_port_implicit() {
        assert_eq!(rdp_target("10.0.0.5", None), "/v:10.0.0.5");
        assert_eq!(rdp_target("10.0.0.5", Some(RDP_DEFAULT_PORT)), "/v:10.0.0.5");
    }
}
