use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use rand::RngCore;

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string();
    password_hash
}

pub fn verify_password(password: &str, hashed_password: &str) -> bool {
    let parsed_hash = PasswordHash::new(hashed_password)
        .expect("Failed to parse password hash");
    Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
}

pub fn encrypt(data: &[u8], key: &[u8; 32]) -> (Vec<u8>, [u8; 12], [u8; 16]) {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    let mut rng = rand::rng();
    rng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext_with_tag = cipher.encrypt(nonce, data)
        .expect("encryption failure!");
    
    // aes-gcm crate appends the tag to the ciphertext
    let tag_pos = ciphertext_with_tag.len() - 16;
    let ciphertext = ciphertext_with_tag[..tag_pos].to_vec();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(&ciphertext_with_tag[tag_pos..]);

    (ciphertext, nonce_bytes, tag)
}

pub fn decrypt(ciphertext: &[u8], key: &[u8; 32], nonce_bytes: &[u8; 12], tag_bytes: &[u8; 16]) -> Result<Vec<u8>, String> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let mut payload = ciphertext.to_vec();
    payload.extend_from_slice(tag_bytes);

    cipher.decrypt(nonce, payload.as_slice())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "my_super_secret_password";
        let hashed = hash_password(password);
        
        assert!(verify_password(password, &hashed));
        assert!(!verify_password("wrong_password", &hashed));
    }

    #[test]
    fn test_encryption_decryption() {
        let data = b"sensitive information";
        let key = [0u8; 32]; // In real use, this would be derived from master password
        
        let (ciphertext, nonce, tag) = encrypt(data, &key);
        let decrypted = decrypt(&ciphertext, &key, &nonce, &tag).unwrap();
        
        assert_eq!(data.to_vec(), decrypted);
    }
}
