//! Local file paths the webview may use for file I/O (audit IPC-001).
//!
//! SFTP upload/download, database export/import and scheduled SFTP tasks read or write local
//! files at a path the frontend supplies. `validate_path` only rejects `..`, so a compromised
//! webview could read any file the user can (and ship it to an SFTP server) or overwrite any
//! file (shell rc files, autostart entries). Now the backend opens the native dialog itself
//! (`pick_local_file` / `pick_save_location`) and records what the user chose; the file-I/O
//! commands only accept paths from that record.
//!
//! Each grant carries the intent of the dialog that produced it (an "open" dialog grants
//! reading, a "save" dialog grants writing) and is consumed by the first command that uses
//! it, so a file the user chose to upload can't later be overwritten, and a pick can't be
//! replayed for the rest of the session.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intent {
    Read,
    Write,
}

#[derive(Default)]
pub struct LocalPathGrants {
    granted: Mutex<HashSet<(PathBuf, Intent)>>,
}

impl LocalPathGrants {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn grant(&self, path: PathBuf, intent: Intent) {
        if let Ok(mut granted) = self.granted.lock() {
            granted.insert((path, intent));
        }
    }

    /// Ok if the user picked exactly this path, for this intent, through a backend-opened
    /// dialog. The grant is used up.
    pub fn take(&self, path: &str, intent: Intent) -> Result<(), String> {
        let mut granted = self
            .granted
            .lock()
            .map_err(|_| "Local path grants are unavailable".to_string())?;
        if granted.remove(&(PathBuf::from(path), intent)) {
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
        assert!(grants.take("/home/u/.ssh/id_ed25519", Intent::Read).is_err());
        grants.grant(PathBuf::from("/home/u/Downloads/report.txt"), Intent::Read);
        assert!(grants.take("/home/u/Downloads/other.txt", Intent::Read).is_err());
        assert!(grants.take("/home/u/Downloads/report.txt.bak", Intent::Read).is_err());
        assert!(grants.take("/home/u/Downloads/report.txt", Intent::Read).is_ok());
    }

    #[test]
    fn grants_are_single_use_and_bound_to_their_intent() {
        let grants = LocalPathGrants::new();
        grants.grant(PathBuf::from("/home/u/report.txt"), Intent::Read);
        // Picked for upload: may not be used as a download/export target.
        assert!(grants.take("/home/u/report.txt", Intent::Write).is_err());
        assert!(grants.take("/home/u/report.txt", Intent::Read).is_ok());
        // Used up.
        assert!(grants.take("/home/u/report.txt", Intent::Read).is_err());
    }
}
