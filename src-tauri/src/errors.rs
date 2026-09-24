use log::error;

/// Sanitizes error messages for frontend display while logging full details server-side.
///
/// This prevents information disclosure through error messages (OWASP ASVS v4.0 7.4.1, CWE-209).
/// Detailed errors are logged server-side for debugging, while generic messages are returned to the frontend.
pub fn sanitize_error(internal_error: String, context: &str) -> String {
    // Log full error server-side for debugging
    error!("[{}] {}", context, internal_error);

    // Return generic error to frontend based on context
    match context {
        "database" => "Database operation failed. Check logs for details.".to_string(),
        "vault" => "Vault operation failed. Check logs for details.".to_string(),
        "ssh" => "SSH operation failed. Check logs for details.".to_string(),
        "network" => "Network operation failed. Check logs for details.".to_string(),
        "sftp" => "File transfer failed. Check logs for details.".to_string(),
        "monitoring" => "Monitoring operation failed. Check logs for details.".to_string(),
        "scanner" => "Network scan operation failed. Check logs for details.".to_string(),
        "credential" => "Credential operation failed. Check logs for details.".to_string(),
        "scheduled task" | "scheduled tasks" => {
            "Scheduled task operation failed. Check logs for details.".to_string()
        }
        "run task" => "Run task failed. Check logs for details.".to_string(),
        "tailscale" => "Tailscale status check failed. Check logs for details.".to_string(),
        _ => "Operation failed. Check logs for details.".to_string(),
    }
}

/// Vault errors the user needs to see verbatim (wrong password, lockout, a blocked password
/// change and why). They carry no internals. Everything else is sanitized as usual.
/// (`unlock_vault` used to map even "Vault is locked out. Try again in N seconds" to a
/// generic failure, so the lockout policy was invisible: audit IPC-012.)
pub fn user_facing_vault_error(internal_error: String) -> String {
    const PASS_THROUGH: &[&str] = &[
        "Invalid master password",
        "Too many failed attempts",
        "Vault is locked",
        "Invalid current password",
        "Master password must",
        "Vault must be unlocked",
        "Vault key does not match",
        "Auto-lock timeout must",
    ];
    if PASS_THROUGH.iter().any(|p| internal_error.starts_with(p))
        || internal_error.contains("can't be decrypted with the current key")
    {
        internal_error
    } else {
        sanitize_error(internal_error, "vault")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_facing_vault_errors_pass_through_and_internals_do_not() {
        let lockout = "Vault is locked out. Try again in 42 seconds".to_string();
        assert_eq!(user_facing_vault_error(lockout.clone()), lockout);
        // The SFTP file manager resolves its credential on every call; after an auto-lock the
        // user must be told to unlock, not "Vault operation failed" (P7-3 review).
        assert_eq!(user_facing_vault_error("Vault is locked".to_string()), "Vault is locked");
        let blocked = "2 credential(s) can't be decrypted with the current key and would be lost".to_string();
        assert_eq!(user_facing_vault_error(blocked.clone()), blocked);
        assert_eq!(
            user_facing_vault_error("Failed to open database: /home/x/quasar.db".to_string()),
            sanitize_error(String::new(), "vault")
        );
    }

    #[test]
    fn test_sanitize_error_does_not_leak_internal_details() {
        let internal =
            "SQLITE error: no such table: credentials at /home/user/.local/quasar.db".to_string();
        let result = sanitize_error(internal.clone(), "database");
        assert!(!result.contains("SQLITE"));
        assert!(!result.contains("/home/user"));
        assert!(!result.contains("credentials"));
        assert!(result.contains("Database operation failed"));
    }

    #[test]
    fn test_sanitize_error_all_contexts() {
        let cases = vec![
            ("database", "Database operation failed"),
            ("vault", "Vault operation failed"),
            ("ssh", "SSH operation failed"),
            ("network", "Network operation failed"),
            ("sftp", "File transfer failed"),
            ("monitoring", "Monitoring operation failed"),
            ("scanner", "Network scan operation failed"),
            ("credential", "Credential operation failed"),
            ("tailscale", "Tailscale status check failed"),
        ];

        for (context, expected_prefix) in cases {
            let result = sanitize_error("internal error".to_string(), context);
            assert!(
                result.starts_with(expected_prefix),
                "Context '{}' should produce message starting with '{}', got '{}'",
                context,
                expected_prefix,
                result
            );
        }
    }

    #[test]
    fn test_sanitize_error_unknown_context_returns_generic() {
        let result = sanitize_error("something broke".to_string(), "unknown_context");
        assert_eq!(result, "Operation failed. Check logs for details.");
        assert!(!result.contains("something broke"));
    }
}
