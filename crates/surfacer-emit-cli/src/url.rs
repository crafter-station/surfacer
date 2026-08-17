//! URL helpers shared across emitters.

pub fn base_url(source_url: &str) -> String {
    url::Url::parse(source_url)
        .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("localhost")))
        .unwrap_or_else(|_| source_url.trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scheme_and_host_from_valid_url() {
        assert_eq!(
            base_url("https://api.example.com/v1/foo"),
            "https://api.example.com"
        );
    }

    #[test]
    fn falls_back_to_localhost_when_host_is_absent() {
        assert_eq!(base_url("file:///foo/bar"), "file://localhost");
    }

    #[test]
    fn trims_trailing_slash_when_source_url_does_not_parse() {
        assert_eq!(base_url("not a url/"), "not a url");
    }
}
