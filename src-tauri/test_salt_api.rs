// Temporary test file to understand Salt API
use argon2::{Argon2, Params, Version};
use password_hash::{Salt, SaltString};

fn main() {
    // Generate a salt
    let salt_string = SaltString::generate(&mut password_hash::rand_core::OsRng);
    println!("SaltString: {}", salt_string.as_str());
    
    // Decode to Salt
    let salt = Salt::from_b64(salt_string.as_str()).unwrap();
    println!("Salt as_str: {}", salt.as_str());
    
    // Check what methods are available
    // The key insight: Salt's as_str() returns the decoded bytes as a &str
    // But those bytes are NOT UTF-8 text - they're binary data
    
    // For Argon2, we need to pass the salt as &[u8]
    // The correct approach is to use salt.as_str().as_bytes()
    // BUT this is only safe because Salt internally stores the bytes as a valid string
    
    let params = Params::new(65536, 3, 4, Some(32)).unwrap();
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    
    let password = b"test_password";
    let mut output = [0u8; 32];
    
    // This is the correct way - salt.as_str() returns &str, .as_bytes() gives &[u8]
    argon2.hash_password_into(password, salt.as_str().as_bytes(), &mut output).unwrap();
    
    println!("Key derived successfully: {:?}", &output[..8]);
}
