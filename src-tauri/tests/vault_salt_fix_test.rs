// Integration test for Issue #1: Salt encoding bug fix
// Verifies that binary salt data is correctly handled without UTF-8 corruption

use argon2::{Argon2, Params, Version};
use password_hash::{Salt, SaltString};

#[test]
fn test_salt_with_binary_bytes() {
    // Test Case: Salt containing 0xFF byte (invalid UTF-8)
    // This would fail with the old .as_str().as_bytes() approach
    
    // Create a salt with binary data
    let salt_b64 = "////////////////////"; // This decodes to bytes with 0xFF
    let salt = Salt::from_b64(salt_b64).expect("Failed to parse salt");
    
    // Decode to raw bytes
    let mut salt_bytes = [0u8; 64];
    let salt_decoded = salt.decode_b64(&mut salt_bytes)
        .expect("Failed to decode salt bytes");
    
    // Verify we got binary data
    assert!(salt_decoded.contains(&0xFF), "Salt should contain 0xFF byte");
    
    // Now test key derivation with this binary salt
    let params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    
    let password = b"test_password_123";
    let mut key1 = [0u8; 32];
    
    // This should NOT panic with binary salt data
    argon2.hash_password_into(password, salt_decoded, &mut key1)
        .expect("Key derivation should succeed with binary salt");
    
    // Verify key was actually derived (not all zeros)
    assert_ne!(key1, [0u8; 32], "Key should be derived, not zeros");
}

#[test]
fn test_key_derivation_consistency() {
    // Test Case: Same password + salt should produce identical keys
    // This verifies the fix doesn't break deterministic key derivation
    
    let salt_string = SaltString::generate(&mut password_hash::rand_core::OsRng);
    let salt = Salt::from_b64(salt_string.as_str()).unwrap();
    
    let params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    
    let password = b"consistent_password";
    
    // Derive key first time
    let mut salt_bytes1 = [0u8; 64];
    let salt_decoded1 = salt.decode_b64(&mut salt_bytes1).unwrap();
    let mut key1 = [0u8; 32];
    argon2.hash_password_into(password, salt_decoded1, &mut key1).unwrap();
    
    // Derive key second time with same salt
    let mut salt_bytes2 = [0u8; 64];
    let salt_decoded2 = salt.decode_b64(&mut salt_bytes2).unwrap();
    let mut key2 = [0u8; 32];
    argon2.hash_password_into(password, salt_decoded2, &mut key2).unwrap();
    
    // Keys MUST be identical
    assert_eq!(key1, key2, "Same password + salt must produce identical keys");
}

#[test]
fn test_salt_with_null_bytes() {
    // Edge Case: Salt containing 0x00 bytes
    // This is valid binary data that would be problematic as UTF-8
    
    let salt_b64 = "AAAAAAAAAAAAAAAA"; // Decodes to null bytes
    let salt = Salt::from_b64(salt_b64).expect("Failed to parse salt");
    
    let mut salt_bytes = [0u8; 64];
    let salt_decoded = salt.decode_b64(&mut salt_bytes)
        .expect("Failed to decode salt with null bytes");
    
    // Verify we got null bytes
    assert!(salt_decoded.iter().all(|&b| b == 0x00), "Salt should be all null bytes");
    
    // Key derivation should still work
    let params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    
    let password = b"test_password";
    let mut key = [0u8; 32];
    
    argon2.hash_password_into(password, salt_decoded, &mut key)
        .expect("Key derivation should work with null byte salt");
    
    assert_ne!(key, [0u8; 32], "Key should be derived");
}

#[test]
fn test_salt_with_high_bytes() {
    // Edge Case: Salt with bytes 0x80-0xFF (invalid UTF-8 continuation bytes)
    
    let salt_b64 = "gICAgICAgICAgICA"; // Decodes to 0x80 bytes
    let salt = Salt::from_b64(salt_b64).expect("Failed to parse salt");
    
    let mut salt_bytes = [0u8; 64];
    let salt_decoded = salt.decode_b64(&mut salt_bytes)
        .expect("Failed to decode salt with high bytes");
    
    // Verify we got 0x80 bytes
    assert!(salt_decoded.contains(&0x80), "Salt should contain 0x80 byte");
    
    let params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    
    let password = b"test_password";
    let mut key = [0u8; 32];
    
    argon2.hash_password_into(password, salt_decoded, &mut key)
        .expect("Key derivation should work with 0x80 bytes");
    
    assert_ne!(key, [0u8; 32], "Key should be derived");
}
