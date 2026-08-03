//! SSH authentication: password or public key (key path or PEM string).

use russh::client::Handler;
use russh::client::{AuthResult, Handle};
use russh::keys::{decode_secret_key, PrivateKeyWithHashAlg};
use std::path::Path;
use std::sync::Arc;

/// Resolve private key PEM string from key_path (read file) or private_key (use as-is).
pub fn resolve_key_material(
    key_path: Option<&str>,
    private_key: Option<&str>,
) -> Result<Option<String>, String> {
    let key_pem = match (key_path, private_key) {
        (Some(path), _) => {
            let path = expand_tilde(path);
            let path = Path::new(&path);
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read key file: {}", e))?
        }
        (_, Some(pem)) => pem.to_string(),
        (None, None) => return Ok(None),
    };
    Ok(Some(key_pem))
}

/// Call after resolving key material: (key_pem, passphrase) from credential.
pub fn parse_private_key(
    key_pem: &str,
    passphrase: Option<&str>,
) -> Result<russh::keys::PrivateKey, String> {
    decode_secret_key(key_pem, passphrase).map_err(|e| {
        log::error!("SSH key parse error: {}", e);
        "SSH key authentication failed".to_string()
    })
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return home.to_string_lossy().to_string() + &path[1..];
        }
        #[cfg(windows)]
        if let Some(home) = std::env::var_os("USERPROFILE") {
            return home.to_string_lossy().to_string() + &path[1..];
        }
    }
    path.to_string()
}

/// Authenticate the session with either password or SSH key.
/// If both are provided, tries key first then password.
pub async fn authenticate<H: Handler + Send + 'static>(
    session: &mut Handle<H>,
    username: &str,
    password: Option<&str>,
    key_path: Option<&str>,
    private_key: Option<&str>,
    key_passphrase: Option<&str>,
) -> Result<(), String> {
    // Try key auth first if we have key material
    if let Some(key_pem) = resolve_key_material(key_path, private_key)? {
        let key = parse_private_key(&key_pem, key_passphrase)?;
        let hash_alg = session
            .best_supported_rsa_hash()
            .await
            .ok()
            .flatten()
            .flatten();
        let key_with_alg = PrivateKeyWithHashAlg::new(Arc::new(key), hash_alg);
        let res = session
            .authenticate_publickey(username, key_with_alg)
            .await
            .map_err(|e| e.to_string())?;
        if matches!(res, AuthResult::Success) {
            return Ok(());
        }
        // Key auth failed (e.g. key not accepted); fall through to password if provided
    }

    if let Some(pass) = password {
        let res = session
            .authenticate_password(username, pass)
            .await
            .map_err(|e| e.to_string())?;
        if matches!(res, AuthResult::Success) {
            return Ok(());
        }
        return Err("Authentication failed".to_string());
    }

    Err("No password or SSH key provided".to_string())
}
