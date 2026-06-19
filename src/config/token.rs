//! Token generation utilities.

use std::fmt::Write as _;

/// Generate a cryptographically random 32-character hex token (128 bits of entropy).
///
/// Backed by the OS CSPRNG ([`getrandom`]), so it is safe to use as an auth secret.
///
/// # Example
///
/// ```rust
/// use mcp_core::generate_random_token;
///
/// let token = generate_random_token();
/// assert_eq!(token.len(), 32);
/// assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
/// ```
pub fn generate_random_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS CSPRNG (getrandom) unavailable");
    let mut token = String::with_capacity(32);
    for byte in bytes {
        let _ = write!(token, "{byte:02x}");
    }
    token
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_length() {
        let token = generate_random_token();
        assert_eq!(token.len(), 32);
    }

    #[test]
    fn test_token_is_hex() {
        let token = generate_random_token();
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_tokens_are_unique() {
        let token1 = generate_random_token();
        let token2 = generate_random_token();
        assert_ne!(token1, token2);
    }
}
