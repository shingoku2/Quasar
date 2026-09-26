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

use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// Error returned when the user declines the native dialog.
pub const DECLINED: &str = "Cancelled";

/// An answer faster than this can't come from someone who read the dialog. The confirming
/// button is the default one on macOS/Windows, so a compromised webview could open a dialog
/// just as the user presses Enter in the terminal; such an answer counts as a decline.
/// (The dialog API can't make Cancel the default: on Linux a closed dialog reports the same
/// result as the second button, so swapping the buttons would let Esc confirm.)
const MIN_HUMAN_RESPONSE: Duration = Duration::from_millis(800);

/// After a decline, new confirmations are refused for this long, so a loop of requests
/// can't bury the user in dialogs until one gets clicked through.
const DECLINE_COOLDOWN: Duration = Duration::from_secs(3);

/// One dialog at a time; `last_decline` drives the cooldown.
struct DialogGate {
    open: bool,
    last_decline: Option<Instant>,
}

static GATE: Mutex<DialogGate> = Mutex::new(DialogGate { open: false, last_decline: None });

fn begin_dialog() -> Result<(), String> {
    let mut gate = GATE.lock().unwrap_or_else(|p| p.into_inner());
    if gate.open {
        return Err("Another confirmation is already open".to_string());
    }
    if gate.last_decline.is_some_and(|t| t.elapsed() < DECLINE_COOLDOWN) {
        return Err(DECLINED.to_string());
    }
    gate.open = true;
    Ok(())
}

fn end_dialog(confirmed: bool) {
    let mut gate = GATE.lock().unwrap_or_else(|p| p.into_inner());
    gate.open = false;
    if !confirmed {
        gate.last_decline = Some(Instant::now());
    }
}

/// Whether a dialog answer counts as a confirmation.
fn accepted(clicked_ok: bool, answered_after: Duration) -> bool {
    clicked_ok && answered_after >= MIN_HUMAN_RESPONSE
}

/// Makes webview-controlled text (credential names, hosts) safe to show in a dialog: no
/// control characters or line breaks that could fake dialog text, and a bounded length.
pub fn display_text(s: &str) -> String {
    const MAX_CHARS: usize = 64;
    let cleaned: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.chars().count() > MAX_CHARS {
        format!("{}…", cleaned.chars().take(MAX_CHARS).collect::<String>())
    } else {
        cleaned.to_string()
    }
}

/// Shows a native OK/Cancel dialog and returns `Err(DECLINED)` unless the user confirms.
/// Only one dialog is open at a time, requests right after a decline are refused, and an
/// instant answer is treated as a decline (see `MIN_HUMAN_RESPONSE`).
pub async fn confirm(app: &AppHandle, title: &str, message: String, ok_label: &str) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    begin_dialog()?;
    let shown_at = Instant::now();
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
    .await;
    let confirmed = matches!(confirmed, Ok(true)) && accepted(true, shown_at.elapsed());
    end_dialog(confirmed);
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
/// `current` is the stored status for the host and port (`None` when no key is stored).
/// Trusting always asks. So does any change out of `Rejected`: moving it to `unknown` or
/// `changed` would turn a refusal into an ordinary prompt the webview can approve itself
/// (PR #68 review).
pub fn trust_change_needs_confirm(
    current: Option<&crate::vault::TrustStatus>,
    requested: &crate::vault::TrustStatus,
) -> bool {
    use crate::vault::TrustStatus;
    matches!(requested, TrustStatus::Trusted)
        || (matches!(current, Some(TrustStatus::Rejected)) && !matches!(requested, TrustStatus::Rejected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::TrustStatus;

    #[test]
    fn instant_answers_do_not_confirm() {
        assert!(!accepted(true, Duration::from_millis(50)), "an Enter already in flight");
        assert!(accepted(true, MIN_HUMAN_RESPONSE));
        assert!(!accepted(false, Duration::from_secs(10)));
    }

    #[test]
    fn one_dialog_at_a_time_and_a_cooldown_after_decline() {
        begin_dialog().unwrap();
        assert!(begin_dialog().is_err(), "a second dialog is refused while one is open");
        end_dialog(false);
        assert_eq!(begin_dialog().unwrap_err(), DECLINED, "refused during the cooldown");
        GATE.lock().unwrap().last_decline = None;
        begin_dialog().unwrap();
        end_dialog(true);
    }

    #[test]
    fn dialog_text_from_the_webview_is_neutralised() {
        let fake = "x'?\n\nRoutine re-check. Click Reveal to continue.";
        assert!(!display_text(fake).contains('\n'));
        assert_eq!(display_text(&"a".repeat(100)).chars().count(), 65);
        assert_eq!(display_text("  db1\t "), "db1");
    }

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
        for current in [None, Some(TrustStatus::Trusted), Some(TrustStatus::Unknown), Some(TrustStatus::Changed)] {
            assert!(!trust_change_needs_confirm(current.as_ref(), &TrustStatus::Rejected));
            assert!(!trust_change_needs_confirm(current.as_ref(), &TrustStatus::Unknown));
            assert!(!trust_change_needs_confirm(current.as_ref(), &TrustStatus::Changed));
            assert!(trust_change_needs_confirm(current.as_ref(), &TrustStatus::Trusted));
        }
        // PR #68 review: leaving Rejected in any direction asks.
        let rejected = Some(TrustStatus::Rejected);
        assert!(trust_change_needs_confirm(rejected.as_ref(), &TrustStatus::Unknown));
        assert!(trust_change_needs_confirm(rejected.as_ref(), &TrustStatus::Changed));
        assert!(trust_change_needs_confirm(rejected.as_ref(), &TrustStatus::Trusted));
        assert!(!trust_change_needs_confirm(rejected.as_ref(), &TrustStatus::Rejected));
    }
}
