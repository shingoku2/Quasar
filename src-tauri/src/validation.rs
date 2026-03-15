//! Input validation utilities for Quasar
//!
//! This module provides validation functions for user inputs to prevent
//! injection attacks and ensure data integrity (OWASP ASVS v4.0 5.1.3).
//!
//! All validation functions return `Result<(), String>` where:
//! - `Ok(())` indicates valid input
//! - `Err(String)` contains a user-friendly error message

use regex::Regex;
use once_cell::sync::Lazy;

/// Validates IPv4 address format.
///
/// Accepts only standard dotted-decimal notation with each octet in 0–255.
/// Leading zeros are rejected to avoid octal ambiguity (RFC 3986 §3.2.2).
///
/// # Examples
/// ```ignore
/// assert!(validate_ip("192.168.1.1").is_ok());
/// assert!(validate_ip("256.1.1.1").is_err());
/// assert!(validate_ip("001.002.003.004").is_err()); // leading zeros rejected
/// ```
pub fn validate_ip(ip: &str) -> Result<(), String> {
    // Strict dotted-decimal: each octet 0-255, no leading zeros, exactly 4 parts.
    static IP_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"^(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)$"
        ).unwrap()
    });

    if IP_REGEX.is_match(ip) {
        Ok(())
    } else {
        Err("Invalid IP address format".to_string())
    }
}

/// Validates port number (1-65535)
pub fn validate_port(port: u16) -> Result<(), String> {
    if port == 0 {
        Err("Port must be between 1 and 65535".to_string())
    } else {
        Ok(())
    }
}

/// Validates hostname format (RFC 1123).
///
/// Each label must be 1-63 alphanumeric characters or hyphens, must not start
/// or end with a hyphen, and the total length must not exceed 253 characters.
pub fn validate_hostname(hostname: &str) -> Result<(), String> {
    static HOSTNAME_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?)*$").unwrap()
    });

    if hostname.is_empty() {
        return Err("Hostname cannot be empty".to_string());
    }

    if hostname.len() > 253 {
        return Err("Hostname too long (max 253 characters)".to_string());
    }

    if HOSTNAME_REGEX.is_match(hostname) {
        Ok(())
    } else {
        Err("Invalid hostname format".to_string())
    }
}

/// Validates username (alphanumeric, underscore, hyphen, dot, 1-64 chars)
pub fn validate_username(username: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("Username cannot be empty".to_string());
    }

    if username.len() > 64 {
        return Err("Username too long (max 64 characters)".to_string());
    }

    static USERNAME_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"^[a-zA-Z0-9_\-\.]+$").unwrap()
    });

    if USERNAME_REGEX.is_match(username) {
        Ok(())
    } else {
        Err("Username must contain only alphanumeric characters, underscores, hyphens, or dots".to_string())
    }
}

/// Validates CIDR notation
pub fn validate_cidr(cidr: &str) -> Result<(), String> {
    static CIDR_REGEX: Lazy<Regex> = Lazy::new(|| {
        Regex::new(
            r"^(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]\d|\d)/([0-9]|[1-2][0-9]|3[0-2])$"
        ).unwrap()
    });

    if CIDR_REGEX.is_match(cidr) {
        Ok(())
    } else {
        Err("Invalid CIDR notation".to_string())
    }
}

/// Validates credential name (non-empty, max 100 chars)
pub fn validate_credential_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Credential name cannot be empty".to_string());
    }

    if name.len() > 100 {
        return Err("Credential name too long (max 100 characters)".to_string());
    }

    Ok(())
}

/// Validates password strength (min 8 chars for regular passwords)
#[allow(dead_code)]
pub fn validate_password(password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters".to_string());
    }

    Ok(())
}

/// Validates master password strength (min 12 chars, already enforced in vault)
pub fn validate_master_password(password: &str) -> Result<(), String> {
    if password.len() < 12 {
        return Err("Master password must be at least 12 characters".to_string());
    }

    Ok(())
}

/// Validates a file-system path supplied by the user.
///
/// Rejects paths containing null bytes or `..` path traversal components.
/// Does not check existence; callers must do that separately.
pub fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    if path.contains('\0') {
        return Err("Path contains invalid characters".to_string());
    }
    // Reject any component that is exactly ".." to prevent directory traversal.
    let traversal = std::path::Path::new(path)
        .components()
        .any(|c| c == std::path::Component::ParentDir);
    if traversal {
        return Err("Path must not contain '..' components".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_ip() {
        assert!(validate_ip("192.168.1.1").is_ok());
        assert!(validate_ip("10.0.0.1").is_ok());
        assert!(validate_ip("255.255.255.255").is_ok());
        assert!(validate_ip("0.0.0.0").is_ok());
        assert!(validate_ip("256.1.1.1").is_err());
        assert!(validate_ip("192.168.1").is_err());
        assert!(validate_ip("not-an-ip").is_err());
        // Leading zeros must be rejected
        assert!(validate_ip("001.002.003.004").is_err());
        assert!(validate_ip("192.168.01.1").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(22).is_ok());
        assert!(validate_port(80).is_ok());
        assert!(validate_port(65535).is_ok());
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn test_validate_hostname() {
        assert!(validate_hostname("example.com").is_ok());
        assert!(validate_hostname("sub.example.com").is_ok());
        assert!(validate_hostname("localhost").is_ok());
        assert!(validate_hostname("").is_err());
        assert!(validate_hostname("invalid..hostname").is_err());
    }

    #[test]
    fn test_validate_username() {
        assert!(validate_username("user").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("user-name").is_ok());
        assert!(validate_username("user123").is_ok());
        assert!(validate_username("john.doe").is_ok());
        assert!(validate_username("_apt").is_ok());
        assert!(validate_username("www-data").is_ok());
        assert!(validate_username("systemd-network").is_ok());
        assert!(validate_username("").is_err());
        assert!(validate_username("user@name").is_err());
        assert!(validate_username(&"a".repeat(65)).is_err());
    }

    #[test]
    fn test_validate_cidr() {
        assert!(validate_cidr("192.168.1.0/24").is_ok());
        assert!(validate_cidr("10.0.0.0/8").is_ok());
        assert!(validate_cidr("172.16.0.0/12").is_ok());
        assert!(validate_cidr("192.168.1.0/33").is_err());
        assert!(validate_cidr("192.168.1.0").is_err());
    }

    #[test]
    fn test_validate_credential_name() {
        assert!(validate_credential_name("Production Server").is_ok());
        assert!(validate_credential_name("a").is_ok());
        assert!(validate_credential_name("").is_err());
        assert!(validate_credential_name(&"a".repeat(101)).is_err());
    }

    #[test]
    fn test_validate_password() {
        assert!(validate_password("password123").is_ok());
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password("short").is_err());
    }

    #[test]
    fn test_validate_master_password() {
        assert!(validate_master_password("SecurePass123!").is_ok());
        assert!(validate_master_password("short").is_err());
        assert!(validate_master_password("11chars!!!!").is_err());
    }

    #[test]
    fn test_validate_path() {
        assert!(validate_path("/home/user/file.txt").is_ok());
        assert!(validate_path("C:\\Users\\file.txt").is_ok());
        assert!(validate_path("relative/path/file.txt").is_ok());
        assert!(validate_path("").is_err());
        assert!(validate_path("/home/user/../etc/passwd").is_err());
        assert!(validate_path("../secret").is_err());
        assert!(validate_path("file\0name").is_err());
    }
}
