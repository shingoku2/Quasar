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
        _ => "Operation failed. Check logs for details.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
