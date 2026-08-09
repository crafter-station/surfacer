use anyhow::{anyhow, Result};
use surfacer_ir::TokenCapture;
use url::Url;

use crate::har::HarLog;

/// Pull a captured token out of a recorded HAR, per the descriptor's
/// `TokenCapture`.
///
/// Only `RequestQueryParam` and `ResponseHeader` are observable in a HAR
/// (which models requests and responses, not `document.cookie` or
/// `localStorage`). `Cookie` and `Storage` need a different capture surface
/// (`agent-browser eval`) and return a clear "not yet implemented" error so a
/// caller never silently gets `None` for an unsupported variant.
pub fn extract_token(har: &HarLog, capture: &TokenCapture) -> Result<Option<String>> {
    match capture {
        TokenCapture::RequestQueryParam {
            url_contains,
            param,
        } => Ok(extract_request_query_param(har, url_contains, param)),
        TokenCapture::ResponseHeader {
            url_contains,
            header,
        } => Ok(extract_response_header(har, url_contains, header)),
        TokenCapture::Cookie { .. } => Err(anyhow!(
            "token capture from cookies is not yet implemented for this capture kind (needs agent-browser eval of document.cookie, not the HAR)"
        )),
        TokenCapture::Storage { .. } => Err(anyhow!(
            "token capture from browser storage is not yet implemented for this capture kind (needs agent-browser eval of localStorage/sessionStorage, not the HAR)"
        )),
    }
}

/// First request whose URL contains `url_contains`, returning the value of the
/// `param` query parameter. The SUNAT idCache case: it rides `idCache=` on the
/// `servletAcceso` navigation.
fn extract_request_query_param(har: &HarLog, url_contains: &str, param: &str) -> Option<String> {
    for entry in &har.log.entries {
        let url = &entry.request.url;
        if !url.contains(url_contains) {
            continue;
        }
        let Ok(parsed) = Url::parse(url) else {
            continue;
        };
        if let Some((_, value)) = parsed.query_pairs().find(|(key, _)| key == param) {
            return Some(value.into_owned());
        }
    }
    None
}

/// First request whose URL contains `url_contains`, returning the value of the
/// named response header (case-insensitive on the header name).
fn extract_response_header(har: &HarLog, url_contains: &str, header: &str) -> Option<String> {
    for entry in &har.log.entries {
        if !entry.request.url.contains(url_contains) {
            continue;
        }
        if let Some(found) = entry
            .response
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(header))
        {
            return Some(found.value.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::har::parse_har;
    use surfacer_ir::BrowserStore;

    fn har_with_url(url: &str) -> HarLog {
        let json = format!(
            r#"{{"log":{{"version":"1.2","entries":[{{"request":{{"method":"GET","url":"{url}"}}}}]}}}}"#
        );
        parse_har(json.as_bytes()).expect("parse har")
    }

    #[test]
    fn extracts_idcache_query_param_from_request_url() {
        let har = har_with_url(
            "https://e-plataformaunica.sunat.gob.pe/servletAcceso?state=x&idCache=eyJhbGciOiJIUzI1NiJ9.payload.sig",
        );
        let capture = TokenCapture::RequestQueryParam {
            url_contains: "servletAcceso".into(),
            param: "idCache".into(),
        };
        let token = extract_token(&har, &capture).unwrap();
        assert_eq!(
            token.as_deref(),
            Some("eyJhbGciOiJIUzI1NiJ9.payload.sig")
        );
    }

    #[test]
    fn returns_none_when_no_matching_request() {
        let har = har_with_url("https://example.test/other?foo=bar");
        let capture = TokenCapture::RequestQueryParam {
            url_contains: "servletAcceso".into(),
            param: "idCache".into(),
        };
        assert!(extract_token(&har, &capture).unwrap().is_none());
    }

    #[test]
    fn returns_none_when_param_absent_on_matched_url() {
        let har = har_with_url("https://host.test/servletAcceso?state=x");
        let capture = TokenCapture::RequestQueryParam {
            url_contains: "servletAcceso".into(),
            param: "idCache".into(),
        };
        assert!(extract_token(&har, &capture).unwrap().is_none());
    }

    #[test]
    fn response_header_capture_reads_header_value() {
        let json = r#"{"log":{"version":"1.2","entries":[
            {"request":{"method":"GET","url":"https://host.test/auth"},
             "response":{"status":200,"headers":[{"name":"X-Token","value":"abc123"}]}}
        ]}}"#;
        let har = parse_har(json.as_bytes()).unwrap();
        let capture = TokenCapture::ResponseHeader {
            url_contains: "/auth".into(),
            header: "x-token".into(),
        };
        assert_eq!(extract_token(&har, &capture).unwrap().as_deref(), Some("abc123"));
    }

    #[test]
    fn cookie_capture_is_unsupported_with_clear_error() {
        let har = har_with_url("https://host.test/login");
        let capture = TokenCapture::Cookie {
            name: "session".into(),
        };
        let err = extract_token(&har, &capture).unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[test]
    fn storage_capture_is_unsupported_with_clear_error() {
        let har = har_with_url("https://host.test/login");
        let capture = TokenCapture::Storage {
            store: BrowserStore::Local,
            key: "access_token".into(),
        };
        let err = extract_token(&har, &capture).unwrap_err();
        assert!(err.to_string().contains("not yet implemented"));
    }
}
