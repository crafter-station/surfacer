//! String escaping shared across emitters.
//!
//! The TypeScript and JavaScript emitters embed recon-derived strings inside
//! generated source. They all escape the same way: backslash and double quote
//! get backslash-escaped, and newlines collapse to spaces so a description can
//! never break out of its string literal.

/// Escape a string for embedding inside a double-quoted TS or JS literal.
///
/// Named for its origin in the TypeScript emitters; the JS emitter (just-bash)
/// needs the identical transform, so this is the one copy.
pub fn escape_ts(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_backslash_quote_and_newlines() {
        assert_eq!(escape_ts(r#"a\b"c"#), r#"a\\b\"c"#);
        assert_eq!(escape_ts("line1\nline2\r"), "line1 line2 ");
    }

    #[test]
    fn leaves_plain_text_untouched() {
        assert_eq!(escape_ts("hola mundo"), "hola mundo");
    }
}
