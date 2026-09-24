//! Local file paths the webview may use for file I/O (audit IPC-001).
//!
//! SFTP upload/download, database export/import and scheduled SFTP tasks read or write local
//! files at a path the frontend supplies. `validate_path` only rejects `..`, so a compromised
//! webview could read any file the user can (and ship it to an SFTP server) or overwrite any
//! file (shell rc files, autostart entries). Now the backend opens the native dialog itself
//! (`pick_local_file` / `pick_save_location`) and records what the user chose; the file-I/O
//! commands only accept paths from that record.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct LocalPathGrants {
    granted: Mutex<HashSet<PathBuf>>,
}

impl LocalPathGrants {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&self, path: PathBuf) {
        if let Ok(mut granted) = self.granted.lock() {
            granted.insert(path);
        }
    }

    /// Ok if the user picked exactly this path through a backend-opened dialog.
    pub fn check(&self, path: &str) -> Result<(), String> {
        let granted = self
            .granted
            .lock()
            .map_err(|_| "Local path grants are unavailable".to_string())?;
        if granted.contains(&PathBuf::from(path)) {
            Ok(())
        } else {
            Err("Choose the local file with the file dialog first".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_picked_paths_are_accepted() {
        let grants = LocalPathGrants::new();
        assert!(grants.check("/home/u/.ssh/id_ed25519").is_err());
        grants.grant(PathBuf::from("/home/u/Downloads/report.txt"));
        assert!(grants.check("/home/u/Downloads/report.txt").is_ok());
        assert!(grants.check("/home/u/Downloads/other.txt").is_err());
        assert!(grants.check("/home/u/Downloads/report.txt.bak").is_err());
    }
}
