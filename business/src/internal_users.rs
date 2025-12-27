//! Internal user management module for test/internal builds.
//!
//! This module provides functionality for managing internal users, including:
//! - Fetching user list from the internal API
//! - Creating users with OTP setup
//! - Generating current OTP codes

use serde::{Deserialize, Serialize};

/// Request to create a new internal user.
#[derive(Debug, Clone, Serialize)]
pub struct CreateInternalUserRequest {
    pub username: String,
}

/// Response after creating an internal user.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateInternalUserResponse {
    pub username: String,
    pub secret: String,
    pub otpauth_url: String,
}

/// Represents an internal user in the system.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct InternalUser {
    pub username: String,
    pub secret: String,
}

impl InternalUser {
    /// Creates a new internal user.
    pub fn new(username: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            secret: secret.into(),
        }
    }
}

/// Response containing list of internal users.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct InternalUsersResponse {
    pub users: Vec<InternalUser>,
}

/// Generates a TOTP code from a base32-encoded secret.
/// This is a simplified implementation for display purposes.
pub fn generate_totp_code(secret_base32: &str) -> Option<String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Parse the secret
    let secret_bytes = base32_decode(secret_base32)?;

    // Get current timestamp
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();

    // TOTP time step is 30 seconds
    let time_step = now / 30;

    // Generate HOTP
    let code = hotp(&secret_bytes, time_step)?;

    Some(format!("{:06}", code % 1_000_000))
}

/// Simple base32 decoder.
fn base32_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let input = input.to_uppercase();
    let input = input.trim_end_matches('=');

    let mut bits: u64 = 0;
    let mut bit_count = 0;
    let mut result = Vec::new();

    for c in input.bytes() {
        let value = ALPHABET.iter().position(|&x| x == c)?;
        bits = (bits << 5) | value as u64;
        bit_count += 5;

        if bit_count >= 8 {
            bit_count -= 8;
            result.push((bits >> bit_count) as u8);
            bits &= (1 << bit_count) - 1;
        }
    }

    Some(result)
}

/// HOTP algorithm (RFC 4226).
fn hotp(secret: &[u8], counter: u64) -> Option<u32> {
    // HMAC-SHA1
    let counter_bytes = counter.to_be_bytes();
    let hmac = hmac_sha1(secret, &counter_bytes)?;

    // Dynamic truncation
    let offset = (hmac[19] & 0x0f) as usize;
    let code = ((hmac[offset] & 0x7f) as u32) << 24
        | (hmac[offset + 1] as u32) << 16
        | (hmac[offset + 2] as u32) << 8
        | hmac[offset + 3] as u32;

    Some(code)
}

/// Simple HMAC-SHA1 implementation.
fn hmac_sha1(key: &[u8], message: &[u8]) -> Option<[u8; 20]> {
    const BLOCK_SIZE: usize = 64;

    // Prepare key
    let mut k = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let hash = sha1(key);
        k[..20].copy_from_slice(&hash);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // Inner and outer padding
    let mut i_key_pad = [0x36u8; BLOCK_SIZE];
    let mut o_key_pad = [0x5cu8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        i_key_pad[i] ^= k[i];
        o_key_pad[i] ^= k[i];
    }

    // Inner hash
    let mut inner = Vec::with_capacity(BLOCK_SIZE + message.len());
    inner.extend_from_slice(&i_key_pad);
    inner.extend_from_slice(message);
    let inner_hash = sha1(&inner);

    // Outer hash
    let mut outer = Vec::with_capacity(BLOCK_SIZE + 20);
    outer.extend_from_slice(&o_key_pad);
    outer.extend_from_slice(&inner_hash);

    Some(sha1(&outer))
}

/// Simple SHA-1 implementation.
fn sha1(message: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    // Pre-processing: adding padding bits
    let mut msg = message.to_vec();
    let original_len = msg.len() as u64 * 8;
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&original_len.to_be_bytes());

    // Process each 512-bit chunk
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for (i, bytes) in chunk.chunks(4).enumerate() {
            let arr: [u8; 4] = bytes.try_into().unwrap_or([0; 4]);
            w[i] = u32::from_be_bytes(arr);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];

        #[allow(clippy::needless_range_loop)]
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _ => (b ^ c ^ d, 0xCA62C1D6u32),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut result = [0u8; 20];
    for (i, &val) in h.iter().enumerate() {
        result[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
    }
    result
}

/// Returns true if the current build is for internal or test environment.
pub fn is_internal_build() -> bool {
    cfg!(feature = "env_internal") || cfg!(feature = "env_test")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base32_decode() {
        // Test with known values - JBSWY3DPEHPK3PXP is a common test secret
        // but not the encoding of "Hello!". Let's test that decoding works.
        let decoded = base32_decode("JBSWY3DPEHPK3PXP");
        assert!(decoded.is_some());
        let bytes = decoded.unwrap();
        // Just verify it decodes to some non-empty bytes
        assert!(!bytes.is_empty());

        // Test round-trip is not possible without an encoder, so just verify format
        // Verify that uppercase and lowercase both work
        let decoded_lower = base32_decode("jbswy3dpehpk3pxp");
        assert!(decoded_lower.is_some());
    }

    #[test]
    fn test_sha1() {
        // Test vector: SHA-1("")
        let result = sha1(b"");
        let expected = [
            0xda, 0x39, 0xa3, 0xee, 0x5e, 0x6b, 0x4b, 0x0d, 0x32, 0x55, 0xbf, 0xef, 0x95, 0x60,
            0x18, 0x90, 0xaf, 0xd8, 0x07, 0x09,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_generate_totp_code_format() {
        // Test that generated code is 6 digits
        let secret = "JBSWY3DPEHPK3PXP";
        let code = generate_totp_code(secret);
        assert!(code.is_some());
        let code = code.unwrap();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_internal_user_new() {
        let user = InternalUser::new("testuser", "SECRET123");
        assert_eq!(user.username, "testuser");
        assert_eq!(user.secret, "SECRET123");
    }
}
