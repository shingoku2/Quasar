
// `launch_ssh` (external terminal) was removed with its never-invoked IPC command
// `launch_ssh_external` (audit IPC-010); the in-app terminal is the only SSH client.

#[cfg(target_os = "windows")]
pub fn launch_rdp(address: &str) -> Result<(), String> {
    std::process::Command::new("mstsc")
        .args(["/v", address])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn launch_rdp(_address: &str) -> Result<(), String> {
    Err("RDP launch only supported on Windows for MVP".to_string())
}
