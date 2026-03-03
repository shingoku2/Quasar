use sha2::{Sha256, Digest};

use argon2::{
    password_hash::{
        rand_core::{OsRng, RngCore},
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2, Algorithm, Params, Version
};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};

pub fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    
    // OWASP-recommended Argon2id parameters: 47 MiB memory, 2 iterations, 1 parallelism
    let params = Params::new(47104, 2, 1, Some(32))
        .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Failed to hash password: {}", e))?
        .to_string();
    Ok(password_hash)
}

pub fn verify_password(password: &str, hashed_password: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hashed_password)
        .map_err(|e| format!("Failed to parse password hash: {}", e))?;
    
    // Use same params for verification (though Argon2 will use params from hash)
    let params = Params::new(47104, 2, 1, Some(32))
        .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    
    Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
}

pub fn encrypt(data: &[u8], key: &[u8; 32]) -> Result<(Vec<u8>, [u8; 12], [u8; 16]), String> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext_with_tag = cipher.encrypt(nonce, data)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    // aes-gcm crate appends the tag to the ciphertext
    let tag_pos = ciphertext_with_tag.len() - 16;
    let ciphertext = ciphertext_with_tag[..tag_pos].to_vec();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&ciphertext_with_tag[tag_pos..]);

    Ok((ciphertext, nonce_bytes, tag))
}

pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce_bytes: &[u8; 12], tag_bytes: &[u8; 16]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let mut payload = ciphertext.to_vec();
    payload.extend_from_slice(tag_bytes);

    cipher.decrypt(nonce, payload.as_slice())
        .map_err(|e| e.to_string())
}

/// Compute an SSH host key fingerprint from raw public key bytes.
///
/// Produces a SHA256 hash formatted as colon-separated hex pairs:
///   `SHA256:aa:bb:cc:dd:ee:ff:...`
///
/// This follows the SSH fingerprint convention defined in RFC 4253 §6.6
/// (hash of the public key blob) using SHA-256 as the digest algorithm,
/// consistent with OpenSSH 6.8+ (`ssh-keygen -l -E sha256`).
///
/// All SSH client modules (interactive, SFTP, exec) MUST use this function
/// to ensure fingerprints are comparable across connection types.
pub fn ssh_host_key_fingerprint(public_key_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(public_key_bytes);
    let hash = hasher.finalize();

    // Format as SHA256:aa:bb:cc:dd:... (colon-separated hex byte pairs)
    let hex_pairs: Vec<String> = hash.iter().map(|b| format!("{:02x}", b)).collect();
    format!("SHA256:{}", hex_pairs.join(":"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "my_super_secret_password";
        let hashed = hash_password(password).unwrap();
        
        assert!(verify_password(password, &hashed).unwrap());
        assert!(!verify_password("wrong_password", &hashed).unwrap());
    }

    #[test]
    fn test_encryption_decryption() {
        let data = b"sensitive information";
        let key = [0u8; 32]; // In real use, this would be derived from master password
        
        let (ciphertext, nonce, tag) = encrypt(data, &key).unwrap();
        let decrypted = decrypt(&ciphertext, &key, &nonce, &tag).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
    }

    #[test]
    fn test_ssh_fingerprint_deterministic() {
        let key_bytes = b"fake-ssh-public-key-data-for-testing";
        let fp1 = ssh_host_key_fingerprint(key_bytes);
        let fp2 = ssh_host_key_fingerprint(key_bytes);
        assert_eq!(fp1, fp2, "Same input must produce identical fingerprints");
    }

    #[test]
    fn test_ssh_fingerprint_format() {
        let key_bytes = b"test-key";
        let fp = ssh_host_key_fingerprint(key_bytes);

        // Must start with SHA256: prefix
        assert!(fp.starts_with("SHA256:"), "Fingerprint must start with SHA256: prefix");

        // After prefix: 32 hex bytes = 32 colon-separated pairs = 31 colons
        let hex_part = &fp["SHA256:".len()..];
        let parts: Vec<&str> = hex_part.split(':').collect();
        assert_eq!(parts.len(), 32, "SHA256 produces 32 bytes = 32 hex pairs");

        // Each part must be exactly 2 hex characters
        for part in &parts {
            assert_eq!(part.len(), 2, "Each segment must be 2 hex chars, got '{}'", part);
            assert!(part.chars().all(|c| c.is_ascii_hexdigit()),
                "Each segment must be hex, got '{}'", part);
        }
    }

    #[test]
    fn test_ssh_fingerprint_different_keys() {
        let fp1 = ssh_host_key_fingerprint(b"key-one");
        let fp2 = ssh_host_key_fingerprint(b"key-two");
        assert_ne!(fp1, fp2, "Different keys must produce different fingerprints");
    }

    #[test]
    fn test_ssh_fingerprint_empty_input() {
        // Empty input should still produce a valid SHA256 fingerprint
        // (SHA256 of empty = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855)
        let fp = ssh_host_key_fingerprint(b"");
        assert!(fp.starts_with("SHA256:"));
        assert_eq!(&fp, "SHA256:e3:b0:c4:42:98:fc:1c:14:9a:fb:f4:c8:99:6f:b9:24:27:ae:41:e4:64:9b:93:4c:a4:95:99:1b:78:52:b8:55");
    }
}
