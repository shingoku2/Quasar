use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};

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
}
