use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

/// A browser-bootstrapped token captured during `auth login`, cached next to
/// the site's `session-name` so a fully headless emitted client can replay it
/// until it expires.
///
/// This is runtime state, not descriptor: it never ships in `ir.json`. It
/// mirrors sunat-cli's `CachedToken` shape
/// (`crafter-research/sunat-cli/packages/cli/src/plataforma/session.ts:27-32`),
/// with the SUNAT-specific `idCache` field renamed to the generic `token`. The
/// camelCase keys (`expiresAt`, `capturedAt`) are a cross-language contract with
/// the TS reader that AE-V4 will emit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TokenCache {
    pub token: String,
    /// Epoch seconds when the token expires, decoded from the JWT `exp` claim.
    pub expires_at: u64,
    /// Epoch seconds when the token was captured.
    pub captured_at: u64,
}

/// Decode the `exp` (epoch seconds) claim out of a JWT without a JWT library,
/// matching sunat-cli's hand-rolled `decodeExp`
/// (`.../plataforma/session.ts:39-46`): split on `.`, base64url-decode the
/// payload segment, parse JSON, read `exp`.
pub fn decode_exp(jwt: &str) -> Result<u64> {
    let payload = jwt
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow!("token is not a JWT (no payload segment)"))?;
    let bytes = base64url_decode(payload)?;
    let claims: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| anyhow!("JWT payload is not JSON: {e}"))?;
    claims
        .get("exp")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow!("JWT has no numeric exp claim"))
}

/// Minimal base64url (RFC 4648 §5) decoder, no padding required. The JWT middle
/// segment is always base64url and unpadded, so this avoids a `base64` crate
/// dep and keeps parity with sunat-cli's zero-dep stance.
fn base64url_decode(input: &str) -> Result<Vec<u8>> {
    fn value(byte: u8) -> Result<u8> {
        match byte {
            b'A'..=b'Z' => Ok(byte - b'A'),
            b'a'..=b'z' => Ok(byte - b'a' + 26),
            b'0'..=b'9' => Ok(byte - b'0' + 52),
            b'-' => Ok(62),
            b'_' => Ok(63),
            b'=' => Err(anyhow!("unexpected padding in base64url payload")),
            other => Err(anyhow!("invalid base64url character: {other:#x}")),
        }
    }

    let cleaned: Vec<u8> = input.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(cleaned.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for byte in cleaned {
        acc = (acc << 6) | u32::from(value(byte)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64_test_helpers::encode_jwt;

    /// Local, test-only base64url encoder so fixtures do not depend on a crate.
    mod base64_test_helpers {
        pub fn encode_jwt(payload_json: &str) -> String {
            let payload = base64url_encode(payload_json.as_bytes());
            // header and signature are irrelevant to decode_exp; use placeholders.
            format!("{}.{}.{}", base64url_encode(b"{}"), payload, "sig")
        }

        fn base64url_encode(input: &[u8]) -> String {
            const ALPHABET: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
            let mut out = String::new();
            for chunk in input.chunks(3) {
                let b0 = chunk[0] as u32;
                let b1 = *chunk.get(1).unwrap_or(&0) as u32;
                let b2 = *chunk.get(2).unwrap_or(&0) as u32;
                let n = (b0 << 16) | (b1 << 8) | b2;
                out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
                out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
                if chunk.len() > 1 {
                    out.push(ALPHABET[((n >> 6) & 63) as usize] as char);
                }
                if chunk.len() > 2 {
                    out.push(ALPHABET[(n & 63) as usize] as char);
                }
            }
            out
        }
    }

    #[test]
    fn decodes_exp_from_jwt_payload() {
        let jwt = encode_jwt(r#"{"aud":"e-plataformaunica","exp":1775880000,"sub":"20123456789"}"#);
        assert_eq!(decode_exp(&jwt).unwrap(), 1_775_880_000);
    }

    #[test]
    fn errors_when_not_a_jwt() {
        let err = decode_exp("notajwt").unwrap_err();
        assert!(err.to_string().contains("not a JWT"));
    }

    #[test]
    fn errors_when_no_exp_claim() {
        let jwt = encode_jwt(r#"{"aud":"x"}"#);
        let err = decode_exp(&jwt).unwrap_err();
        assert!(err.to_string().contains("no numeric exp"));
    }

    #[test]
    fn token_cache_serializes_camel_case() {
        let cache = TokenCache {
            token: "ey.x.y".into(),
            expires_at: 1_775_880_000,
            captured_at: 1_775_876_400,
        };
        let json = serde_json::to_string(&cache).unwrap();
        assert!(json.contains(r#""expiresAt":1775880000"#));
        assert!(json.contains(r#""capturedAt":1775876400"#));
        assert!(json.contains(r#""token":"ey.x.y""#));
        let parsed: TokenCache = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cache);
    }
}
