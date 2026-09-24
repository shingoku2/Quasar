//! Master-password key derivation for the vault.
//!
//! **v2 (current).** One Argon2id pass over the master password produces a 32-byte input
//! key (`ikm`). HKDF-SHA256 then expands it into two independent values:
//! - the AES-256-GCM **encryption key** (`INFO_ENC`), kept only in memory;
//! - the **verifier** (`INFO_VERIFY`), stored hex-encoded in `vault_settings.master_password_verifier`
//!   and compared in constant time on unlock.
//!
//! Knowing the verifier doesn't reveal the encryption key (HKDF outputs with different `info`
//! labels are independent), so a copy of the database only allows an offline guess at the
//! password at full Argon2id cost.
//!
//! **v1 (legacy, pre-2026-09-24).** The vault stored `argon2.hash_password(pw, salt)` as a PHC
//! string and used `argon2.hash_password_into(pw, same salt, 32 bytes)` as the AES key. In
//! argon2 0.5 those are the same bytes, so the stored "hash" *was* the key (audit RSEC-001).
//! v1 vaults are migrated to v2 on their next successful unlock (see `VaultState::unlock_vault`).

use argon2::password_hash::{PasswordHash, PasswordVerifier, Salt};
use argon2::{Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

/// `vault_settings.kdf_version` value for the current scheme. A missing row means v1.
pub const KDF_VERSION_V2: &str = "2";

/// Argon2id parameters (OWASP second-tier baseline: 46 MiB, t=2, p=1). Changing any of these
/// makes every existing vault undecryptable; `kdf_known_answer_*` tests pin them.
const ARGON2_M_COST_KIB: u32 = 47104;
const ARGON2_T_COST: u32 = 2;
const ARGON2_P_COST: u32 = 1;
const KEY_LEN: usize = 32;

const INFO_ENC: &[u8] = b"quasar-vault-v2-encryption-key";
const INFO_VERIFY: &[u8] = b"quasar-vault-v2-password-verifier";

pub fn argon2() -> Result<Argon2<'static>, String> {
    let params = Params::new(ARGON2_M_COST_KIB, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN))
        .map_err(|e| format!("Failed to create Argon2 params: {}", e))?;
    Ok(Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params))
}

/// Argon2id(password, salt) → 32 bytes. `salt_b64` is the B64 salt string stored in
/// `vault_settings.salt`; it's decoded to raw bytes exactly as the v1 code did, so this is
/// also the v1 (legacy) encryption key.
pub fn derive_ikm(password: &[u8], salt_b64: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    let salt = Salt::from_b64(salt_b64).map_err(|e| format!("Failed to parse salt: {}", e))?;
    let mut salt_buf = [0u8; 64];
    let salt_raw = salt
        .decode_b64(&mut salt_buf)
        .map_err(|e| format!("Failed to decode salt bytes: {}", e))?;
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    argon2()?
        .hash_password_into(password, salt_raw, &mut *out)
        .map_err(|e| format!("Failed to derive key: {}", e))?;
    Ok(out)
}

pub struct V2Keys {
    pub enc_key: Zeroizing<[u8; KEY_LEN]>,
    pub verifier: [u8; KEY_LEN],
}

fn expand(ikm: &[u8; KEY_LEN], info: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    Hkdf::<Sha256>::new(None, ikm)
        .expand(info, &mut *out)
        .map_err(|e| format!("HKDF expand failed: {}", e))?;
    Ok(out)
}

pub fn derive_v2(password: &[u8], salt_b64: &str) -> Result<V2Keys, String> {
    let ikm = derive_ikm(password, salt_b64)?;
    let enc_key = expand(&ikm, INFO_ENC)?;
    let verifier = *expand(&ikm, INFO_VERIFY)?;
    Ok(V2Keys { enc_key, verifier })
}

/// Constant-time comparison of a freshly derived verifier against the stored hex value.
/// A malformed stored value never matches.
pub fn verifier_matches(derived: &[u8; KEY_LEN], stored_hex: &str) -> bool {
    match decode_hex(stored_hex) {
        Some(stored) if stored.len() == KEY_LEN => derived.ct_eq(&stored).into(),
        _ => false,
    }
}

/// v1 check against the stored PHC string. Only used to migrate legacy vaults.
pub fn verify_legacy(password: &[u8], stored_phc: &str) -> Result<bool, String> {
    let parsed = PasswordHash::new(stored_phc)
        .map_err(|e| format!("Failed to parse password hash: {}", e))?;
    Ok(argon2()?.verify_password(password, &parsed).is_ok())
}

pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture shared with the audit's standalone reproduction
    // (audit/raw/refresh/rsec001-proof.txt): argon2 0.5.3, same params.
    const PW: &[u8] = b"correct horse battery staple";
    const SALT_B64: &str = "c29tZXNhbHRzb21lc2FsdA";

    /// Pins Argon2id output. If an argon2/password-hash upgrade or a param change alters
    /// this, every existing vault would become undecryptable (audit TEST-005).
    #[test]
    fn kdf_known_answer_argon2_ikm() {
        let ikm = derive_ikm(PW, SALT_B64).unwrap();
        // Independently computed by the scratch crate as the PHC hash field
        // "SGZc7xTufLxRuqf0X5h8MEC5AApJvMMcWQYRBN/oDSE" (unpadded B64).
        assert_eq!(
            encode_hex(&*ikm),
            "48665cef14ee7cbc51baa7f45f987c3040b9000a49bcc31c59061104dfe80d21"
        );
    }

    #[test]
    fn kdf_known_answer_v2_outputs() {
        let keys = derive_v2(PW, SALT_B64).unwrap();
        assert_eq!(encode_hex(&*keys.enc_key), V2_ENC_HEX);
        assert_eq!(encode_hex(&keys.verifier), V2_VERIFY_HEX);
    }

    #[test]
    fn v2_key_and_verifier_are_distinct_from_each_other_and_from_ikm() {
        let ikm = derive_ikm(PW, SALT_B64).unwrap();
        let keys = derive_v2(PW, SALT_B64).unwrap();
        assert_ne!(*keys.enc_key, keys.verifier);
        assert_ne!(*keys.enc_key, *ikm);
        assert_ne!(keys.verifier, *ikm);
    }

    #[test]
    fn verifier_matches_rejects_wrong_or_malformed() {
        let keys = derive_v2(PW, SALT_B64).unwrap();
        let good = encode_hex(&keys.verifier);
        assert!(verifier_matches(&keys.verifier, &good));
        let mut wrong = keys.verifier;
        wrong[0] ^= 1;
        assert!(!verifier_matches(&wrong, &good));
        assert!(!verifier_matches(&keys.verifier, ""));
        assert!(!verifier_matches(&keys.verifier, "zz"));
        assert!(!verifier_matches(&keys.verifier, &good[..62]));
    }

    // Computed independently (Python hmac/hashlib, RFC 5869 HKDF-SHA256, no salt) from the
    // pinned Argon2 output above, so this doesn't just echo the Rust implementation.
    const V2_ENC_HEX: &str = "15b7688270ee5eaf8ce6b8ef544d42381a9a07f80ce26fa3c627acaa0eac38fe";
    const V2_VERIFY_HEX: &str = "dbdb2736ba4bec5ede15327e38009b94077901dac8b6ae3857fb2d06979eec41";
}
