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
        _ => "Operation failed. Check logs for details.".to_string(),
    }
}

/// Sanitizes database-related errors specifically
pub fn sanitize_db_error(internal_error: String) -> String {
    sanitize_error(internal_error, "database")
}

/// Sanitizes vault-related errors specifically
pub fn sanitize_vault_error(internal_error: String) -> String {
    sanitize_error(internal_error, "vault")
}

/// Sanitizes SSH-related errors specifically
pub fn sanitize_ssh_error(internal_error: String) -> String {
    sanitize_error(internal_error, "ssh")
}

/// Sanitizes network-related errors specifically
pub fn sanitize_network_error(internal_error: String) -> String {
    sanitize_error(internal_error, "network")
}
