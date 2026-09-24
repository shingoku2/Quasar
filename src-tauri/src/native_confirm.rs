//! Confirmations the webview can't answer for the user (P7-3 review of IPC-002/IPC-003).
//!
//! Some commands weaken a security decision: accepting a *changed* host key, removing or
//! re-trusting a stored key, loosening a credential's host binding, revealing a plaintext
//! password. A prompt rendered by the webview doesn't protect those, because a compromised
//! webview can skip its own prompt and call the command directly. These commands instead
//! ask through a native OS dialog opened by the backend, which the webview can't click.
//!
//! What stays webview-decided by design: trusting a key for a host that has never been
//! seen (TOFU), and using a credential with no stored host against any host.

use tauri::{AppHandle, Manager};

/// Error returned when the user declines the native dialog.
pub const DECLINED: &str = "Cancelled";

/// Shows a native OK/Cancel dialog and returns `Err(DECLINED)` unless the user confirms.
pub async fn confirm(app: &AppHandle, title: &str, message: String, ok_label: &str) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let app = app.clone();
    let title = title.to_string();
    let ok_label = ok_label.to_string();
    let confirmed = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = app
            .dialog()
            .message(message)
            .title(title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(ok_label, "Cancel".to_string()));
        if let Some(window) = app.get_webview_window("main") {
            builder = builder.parent(&window);
        }
        builder.blocking_show()
    })
    .await
    .map_err(|e| format!("Confirmation dialog failed: {}", e))?;
    if confirmed {
        Ok(())
    } else {
        Err(DECLINED.to_string())
    }
}

/// A credential host update needs confirmation when it loosens or moves an existing
/// binding. Binding an unbound credential only narrows where it can be used.
pub fn host_change_needs_confirm(stored: Option<&str>, requested: Option<&str>) -> bool {
    let norm = |h: Option<&str>| h.map(str::trim).filter(|h| !h.is_empty()).map(str::to_ascii_lowercase);
    let Some(requested) = requested else { return false };
    match norm(stored) {
        None => false,
        Some(bound) => norm(Some(requested)) != Some(bound),
    }
}

/// Marking a stored key trusted lets every path, including unattended ones, connect to that
/// host without a prompt. Any other status only makes the next handshake prompt again.
pub fn trust_change_needs_confirm(status: &crate::vault::TrustStatus) -> bool {
    matches!(status, crate::vault::TrustStatus::Trusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::TrustStatus;

    #[test]
    fn loosening_or_moving_a_binding_needs_confirmation() {
        // No change requested, or binding an unbound credential: narrows only.
        assert!(!host_change_needs_confirm(Some("db1"), None));
        assert!(!host_change_needs_confirm(None, Some("db1")));
        assert!(!host_change_needs_confirm(Some(""), Some("db1")));
        assert!(!host_change_needs_confirm(Some("DB1 "), Some("db1")));
        // Moving or clearing a binding.
        assert!(host_change_needs_confirm(Some("db1"), Some("evil.example")));
        assert!(host_change_needs_confirm(Some("db1"), Some("")));
    }

    #[test]
    fn only_restricting_trust_changes_skip_confirmation() {
        assert!(!trust_change_needs_confirm(&TrustStatus::Rejected));
        assert!(!trust_change_needs_confirm(&TrustStatus::Unknown));
        assert!(!trust_change_needs_confirm(&TrustStatus::Changed));
        assert!(trust_change_needs_confirm(&TrustStatus::Trusted));
    }
}
