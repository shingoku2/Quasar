use std::process::Command;

/// Launch external SSH client. Caller must validate `address` (IP or hostname) and `username` (if present)
/// before calling; see `launch_ssh_external` in lib.rs which validates via validation module.
#[cfg(target_os = "windows")]
pub fn launch_ssh(address: &str, username: Option<&str>) -> Result<(), String> {
    let target = if let Some(user) = username {
        format!("{}@{}", user, address)
    } else {
        address.to_string()
    };

    Command::new("cmd")
        .args(["/C", "start", "ssh", &target])
        .spawn()
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn launch_ssh(address: &str, username: Option<&str>) -> Result<(), String> {
    let target = if let Some(user) = username {
        format!("{}@{}", user, address)
    } else {
        address.to_string()
    };

    // Try to detect terminal emulator? For MVP, let's assume x-terminal-emulator or similar
    // macOS: open ssh://...
    #[cfg(target_os = "macos")]
    {
         Command::new("open")
            .arg(format!("ssh://{}", target))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try common terminal emulators in order of preference.
        let terminals: &[(&str, &[&str])] = &[
            ("x-terminal-emulator", &["-e"]),
            ("gnome-terminal", &["--"]),
            ("konsole", &["-e"]),
            ("xfce4-terminal", &["-e"]),
            ("xterm", &["-e"]),
        ];
        let mut launched = false;
        for (term, flag) in terminals {
            let mut args: Vec<&str> = flag.to_vec();
            args.push("ssh");
            args.push(&target);
            if Command::new(term).args(&args).spawn().is_ok() {
                launched = true;
                break;
            }
        }
        if !launched {
            return Err("No supported terminal emulator found. Install one of: x-terminal-emulator, gnome-terminal, konsole, xfce4-terminal, xterm".to_string());
        }
    }
    
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn launch_rdp(address: &str) -> Result<(), String> {
    Command::new("mstsc")
        .args(["/v", address])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn launch_rdp(_address: &str) -> Result<(), String> {
    Err("RDP launch only supported on Windows for MVP".to_string())
}

#[cfg(test)]
mod tests {
    // These tests are mocked or simple sanity checks as we can't spawn real processes easily in CI/Test
    // without side effects.
    #[test]
    fn test_ssh_target_formatting() {
        let address = "192.168.1.1";
        let user = Some("admin");
        let target = format!("{}@{}", user.unwrap(), address);
        assert_eq!(target, "admin@192.168.1.1");
    }
}
