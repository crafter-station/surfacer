use serde::{Deserialize, Serialize};

use crate::{HttpSurface, OperationDescriptor};

/// How an operation authenticates. Defaults to none so descriptors written
/// before auth existed deserialize unchanged. Internally tagged by `kind`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AuthMode {
    /// No authentication. Explicit so an operation can opt out of a
    /// surface-level default.
    None,

    /// A static secret sent on every request as a header. Covers API keys and
    /// long-lived bearer tokens that the caller already holds. The value is
    /// never in the IR; `secret_ref` names where the client reads it.
    ApiKey(ApiKeyAuth),

    /// OAuth2 where the client acquires a token itself, headless, with no
    /// browser. Password and client-credentials grants. The ordinary case.
    OAuth2(OAuth2Auth),

    /// A token that only the target's own browser session can mint, captured
    /// once and replayed headless until it expires. Acquisition and use are
    /// modeled separately because they happen in different processes at
    /// different times. This is the mode no SDK generator models.
    BrowserBootstrappedToken(BrowserBootstrappedTokenAuth),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyAuth {
    /// Where the value is attached. Header is the common case; query covers
    /// legacy `?apikey=` endpoints observed during recon.
    pub location: CredentialLocation,
    /// The header name or query param name, e.g. "Authorization",
    /// "X-Api-Key", "IdCache".
    pub name: String,
    /// A prefix prepended to the value, e.g. "Bearer ". Empty for a bare key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_prefix: Option<String>,
    /// Names the environment variable or credential file the client reads the
    /// secret from. Never the secret itself (AU-R8).
    pub secret_ref: SecretRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialLocation {
    Header,
    Query,
}

/// Names a secret's source without carrying its value. The client resolves it
/// at runtime; the IR stays safe to commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "from")]
pub enum SecretRef {
    /// Read from an environment variable, e.g. "SUNAT_API_KEY".
    Env { var: String },
    /// Read from a file under the site's config dir, e.g. ".sunat/token".
    File { path: String },
    /// Produced at runtime by an acquisition step (OAuth2 token, captured
    /// browser token). Not read from anywhere static.
    Acquired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuth2Auth {
    pub grant: OAuth2Grant,
    /// The token endpoint observed during recon.
    pub token_url: String,
    /// Scopes seen on the token request, if any.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// How the resulting access token is attached to subsequent requests.
    /// Defaults to `Authorization: Bearer <token>` when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_use: Option<TokenUse>,
    /// Where the grant's own inputs (username/password, client id/secret)
    /// come from. The IR names the sources; the client resolves them.
    pub credentials: SecretRef,
    /// Token lifetime and how to renew. Present when recon observed an
    /// `expires_in` or an equivalent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl: Option<TokenTtl>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuth2Grant {
    Password,
    ClientCredentials,
    /// Only meaningful inside BrowserBootstrappedToken acquisition; kept here
    /// for completeness of the observed grants.
    AuthorizationCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserBootstrappedTokenAuth {
    /// How the token is obtained. Runs in a browser, once per TTL window.
    pub acquire: BrowserAcquisition,
    /// How the captured token is attached to headless requests afterward.
    #[serde(rename = "use")]
    pub use_: TokenUse,
    /// How long the captured token stays valid and when to re-acquire.
    pub ttl: TokenTtl,
}

/// The one-time, browser-side half. Describes what to open and where the
/// token surfaces so the acquisition step can capture it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAcquisition {
    /// URL to open for the human to log in. The portal's own login flow.
    pub login_url: String,
    /// How the token becomes observable once login completes.
    pub capture: TokenCapture,
    /// The named agent-browser session this reuses, tying acquisition to the
    /// existing `surfacer auth login <site>` runtime (AU-R10). `None` means a
    /// fresh session named after the site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<String>,
}

/// Where the token appears during the browser login, so recon and the
/// acquisition step know what to watch for.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "source")]
pub enum TokenCapture {
    /// A query parameter on a specific request, e.g. `idCache=` on the
    /// servletAcceso request. This is the SUNAT F616 case.
    #[serde(rename_all = "camelCase")]
    RequestQueryParam {
        /// A substring the request URL must contain to be the right one.
        url_contains: String,
        /// The query parameter carrying the token, e.g. "idCache".
        param: String,
    },
    /// A response header on a matched request.
    #[serde(rename_all = "camelCase")]
    ResponseHeader {
        url_contains: String,
        header: String,
    },
    /// A cookie set during login.
    #[serde(rename_all = "camelCase")]
    Cookie { name: String },
    /// A value read from browser storage after login, e.g.
    /// localStorage["access_token"].
    #[serde(rename_all = "camelCase")]
    Storage { store: BrowserStore, key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowserStore {
    Local,
    Session,
}

/// How an acquired token is attached to a headless request. Shared by OAuth2
/// and BrowserBootstrappedToken because "use" is the same problem for both:
/// a header (or query param), a name, an optional prefix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUse {
    pub location: CredentialLocation,
    /// Header or query param name, e.g. "Authorization", "IdCache".
    pub name: String,
    /// e.g. "Bearer ". `None` for a bare token like SUNAT's `IdCache`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_prefix: Option<String>,
}

/// Token lifetime and renewal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenTtl {
    /// Observed lifetime in seconds. SUNAT's IdCache and OAuth2 token are both
    /// 3600.
    pub seconds: u64,
    /// What the client does when the token is past its TTL.
    pub on_expiry: RenewalStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenewalStrategy {
    /// Re-run the acquisition automatically. Valid for OAuth2 (headless) but
    /// NOT for BrowserBootstrappedToken, which needs a human.
    Reacquire,
    /// Stop and tell the human to re-run `surfacer auth login <site>`. The
    /// only honest option for a browser-bootstrapped token.
    PromptReauth,
}

/// The auth an operation actually uses: its own override, else the surface
/// default, else None.
pub fn resolve_auth<'a>(
    op: &'a OperationDescriptor,
    surface: Option<&'a HttpSurface>,
) -> Option<&'a AuthMode> {
    op.auth
        .as_ref()
        .or_else(|| surface.and_then(|s| s.auth.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HttpOperation, OperationKind, OperationTransport};

    fn roundtrip(mode: &AuthMode) -> AuthMode {
        let json = serde_json::to_string(mode).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn none_roundtrips() {
        let mode = AuthMode::None;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, r#"{"kind":"none"}"#);
        assert!(matches!(roundtrip(&mode), AuthMode::None));
    }

    #[test]
    fn api_key_roundtrips() {
        let mode = AuthMode::ApiKey(ApiKeyAuth {
            location: CredentialLocation::Header,
            name: "X-Api-Key".into(),
            value_prefix: Some("Bearer ".into()),
            secret_ref: SecretRef::Env {
                var: "SUNAT_API_KEY".into(),
            },
        });
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains(r#""kind":"apiKey""#));
        assert!(json.contains(r#""location":"header""#));
        assert!(json.contains(r#""valuePrefix":"Bearer ""#));
        assert!(json.contains(r#""from":"env""#));
        match roundtrip(&mode) {
            AuthMode::ApiKey(a) => {
                assert_eq!(a.name, "X-Api-Key");
                assert_eq!(a.value_prefix.as_deref(), Some("Bearer "));
                assert!(matches!(a.location, CredentialLocation::Header));
                assert!(matches!(a.secret_ref, SecretRef::Env { var } if var == "SUNAT_API_KEY"));
            }
            other => panic!("expected ApiKey, got {other:?}"),
        }
    }

    #[test]
    fn api_key_omits_absent_prefix() {
        let mode = AuthMode::ApiKey(ApiKeyAuth {
            location: CredentialLocation::Query,
            name: "apikey".into(),
            value_prefix: None,
            secret_ref: SecretRef::File {
                path: ".sunat/token".into(),
            },
        });
        let json = serde_json::to_string(&mode).unwrap();
        assert!(!json.contains("valuePrefix"));
        assert!(json.contains(r#""location":"query""#));
        assert!(json.contains(r#""from":"file""#));
        assert!(matches!(roundtrip(&mode), AuthMode::ApiKey(_)));
    }

    #[test]
    fn oauth2_roundtrips() {
        let mode = AuthMode::OAuth2(OAuth2Auth {
            grant: OAuth2Grant::Password,
            token_url: "https://api-seguridad.sunat.gob.pe/v1/clientessol/{clientId}/oauth2/token/"
                .into(),
            scopes: vec!["sire".into(), "gre".into(), "cpe".into()],
            token_use: None,
            credentials: SecretRef::Env {
                var: "SUNAT_SOL_CREDENTIALS".into(),
            },
            ttl: Some(TokenTtl {
                seconds: 3600,
                on_expiry: RenewalStrategy::Reacquire,
            }),
        });
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains(r#""kind":"oAuth2""#));
        assert!(json.contains(r#""grant":"password""#));
        assert!(json.contains(r#""tokenUrl""#));
        assert!(json.contains(r#""onExpiry":"reacquire""#));
        match roundtrip(&mode) {
            AuthMode::OAuth2(o) => {
                assert!(matches!(o.grant, OAuth2Grant::Password));
                assert_eq!(o.scopes, vec!["sire", "gre", "cpe"]);
                assert_eq!(o.ttl.as_ref().map(|t| t.seconds), Some(3600));
                assert!(o.token_use.is_none());
            }
            other => panic!("expected OAuth2, got {other:?}"),
        }
    }

    #[test]
    fn oauth2_omits_empty_scopes_and_absent_optionals() {
        let mode = AuthMode::OAuth2(OAuth2Auth {
            grant: OAuth2Grant::ClientCredentials,
            token_url: "https://example.test/token".into(),
            scopes: Vec::new(),
            token_use: None,
            credentials: SecretRef::Acquired,
            ttl: None,
        });
        let json = serde_json::to_string(&mode).unwrap();
        assert!(!json.contains("scopes"));
        assert!(!json.contains("tokenUse"));
        assert!(!json.contains("ttl"));
        assert!(json.contains(r#""grant":"clientCredentials""#));
        assert!(json.contains(r#""from":"acquired""#));
        assert!(matches!(roundtrip(&mode), AuthMode::OAuth2(_)));
    }

    #[test]
    fn browser_bootstrapped_token_roundtrips() {
        let mode = AuthMode::BrowserBootstrappedToken(BrowserBootstrappedTokenAuth {
            acquire: BrowserAcquisition {
                login_url: "https://e-menu.sunat.gob.pe/cl-ti-itmenu/AutenticaMenuInternet.htm"
                    .into(),
                capture: TokenCapture::RequestQueryParam {
                    url_contains: "servletAcceso".into(),
                    param: "idCache".into(),
                },
                session_ref: Some("sunat".into()),
            },
            use_: TokenUse {
                location: CredentialLocation::Header,
                name: "IdCache".into(),
                value_prefix: None,
            },
            ttl: TokenTtl {
                seconds: 3600,
                on_expiry: RenewalStrategy::PromptReauth,
            },
        });
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains(r#""kind":"browserBootstrappedToken""#));
        assert!(json.contains(r#""source":"requestQueryParam""#));
        assert!(json.contains(r#""urlContains":"servletAcceso""#));
        assert!(json.contains(r#""onExpiry":"promptReauth""#));
        match roundtrip(&mode) {
            AuthMode::BrowserBootstrappedToken(b) => {
                assert_eq!(b.use_.name, "IdCache");
                assert_eq!(b.ttl.seconds, 3600);
                assert!(matches!(b.ttl.on_expiry, RenewalStrategy::PromptReauth));
                assert!(matches!(
                    b.acquire.capture,
                    TokenCapture::RequestQueryParam { .. }
                ));
                assert_eq!(b.acquire.session_ref.as_deref(), Some("sunat"));
            }
            other => panic!("expected BrowserBootstrappedToken, got {other:?}"),
        }
    }

    #[test]
    fn use_field_serializes_as_bare_use_key() {
        // `use` is a Rust keyword, so the field is `use_` with an explicit
        // rename. The serialized JSON key MUST be exactly "use" (matching the
        // doc's SUNAT example), never "use_".
        let mode = AuthMode::BrowserBootstrappedToken(BrowserBootstrappedTokenAuth {
            acquire: BrowserAcquisition {
                login_url: "https://example.test/login".into(),
                capture: TokenCapture::Cookie {
                    name: "session".into(),
                },
                session_ref: None,
            },
            use_: TokenUse {
                location: CredentialLocation::Header,
                name: "IdCache".into(),
                value_prefix: None,
            },
            ttl: TokenTtl {
                seconds: 3600,
                on_expiry: RenewalStrategy::PromptReauth,
            },
        });
        let json = serde_json::to_string(&mode).unwrap();
        assert!(json.contains(r#""use":"#), "expected bare `use` key: {json}");
        assert!(!json.contains(r#""use_""#), "must not emit `use_`: {json}");
        assert!(!json.contains("sessionRef"), "None session ref omitted");
    }

    #[test]
    fn token_capture_variants_roundtrip() {
        let captures = vec![
            TokenCapture::RequestQueryParam {
                url_contains: "servletAcceso".into(),
                param: "idCache".into(),
            },
            TokenCapture::ResponseHeader {
                url_contains: "/auth".into(),
                header: "X-Token".into(),
            },
            TokenCapture::Cookie {
                name: "session".into(),
            },
            TokenCapture::Storage {
                store: BrowserStore::Local,
                key: "access_token".into(),
            },
        ];
        for capture in captures {
            let json = serde_json::to_string(&capture).unwrap();
            let parsed: TokenCapture = serde_json::from_str(&json).unwrap();
            let reparsed = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, reparsed);
        }
        // Storage store serializes camelCase.
        let storage = TokenCapture::Storage {
            store: BrowserStore::Session,
            key: "k".into(),
        };
        let json = serde_json::to_string(&storage).unwrap();
        assert!(json.contains(r#""store":"session""#));
    }

    fn op_with_auth(auth: Option<AuthMode>) -> OperationDescriptor {
        OperationDescriptor {
            command_path: vec!["ficha-ruc".into()],
            summary: "s".into(),
            description: "d".into(),
            operation_kind: OperationKind::Read,
            transport: OperationTransport::Http(HttpOperation { endpoint_index: 0 }),
            extractor: None,
            auth,
        }
    }

    #[test]
    fn resolve_auth_operation_override_wins() {
        let op = op_with_auth(Some(AuthMode::None));
        let surface = HttpSurface {
            endpoints: Vec::new(),
            auth: Some(AuthMode::OAuth2(OAuth2Auth {
                grant: OAuth2Grant::Password,
                token_url: "https://example.test/token".into(),
                scopes: Vec::new(),
                token_use: None,
                credentials: SecretRef::Acquired,
                ttl: None,
            })),
        };
        let resolved = resolve_auth(&op, Some(&surface));
        assert!(matches!(resolved, Some(AuthMode::None)));
    }

    #[test]
    fn resolve_auth_falls_back_to_surface_default() {
        let op = op_with_auth(None);
        let surface = HttpSurface {
            endpoints: Vec::new(),
            auth: Some(AuthMode::OAuth2(OAuth2Auth {
                grant: OAuth2Grant::Password,
                token_url: "https://example.test/token".into(),
                scopes: Vec::new(),
                token_use: None,
                credentials: SecretRef::Acquired,
                ttl: None,
            })),
        };
        let resolved = resolve_auth(&op, Some(&surface));
        assert!(matches!(resolved, Some(AuthMode::OAuth2(_))));
    }

    #[test]
    fn resolve_auth_none_when_no_op_and_no_surface() {
        let op = op_with_auth(None);
        assert!(resolve_auth(&op, None).is_none());

        let surface = HttpSurface {
            endpoints: Vec::new(),
            auth: None,
        };
        assert!(resolve_auth(&op, Some(&surface)).is_none());
    }

    #[test]
    fn legacy_operation_without_auth_field_deserializes() {
        // Forward-compat guarantee (AU-R0): a descriptor written before auth
        // existed has no `auth` key and MUST still parse.
        let legacy = r#"{
            "commandPath": ["ficha-ruc"],
            "summary": "Consulta ficha RUC",
            "description": "d",
            "operationKind": "read",
            "transport": { "kind": "http", "endpointIndex": 1 }
        }"#;
        let op: OperationDescriptor = serde_json::from_str(legacy).unwrap();
        assert!(op.auth.is_none());
        assert_eq!(op.command_path, vec!["ficha-ruc"]);
    }

    #[test]
    fn legacy_http_surface_without_auth_field_deserializes() {
        // Same forward-compat guarantee at the surface level.
        let legacy = r#"{ "endpoints": [] }"#;
        let surface: HttpSurface = serde_json::from_str(legacy).unwrap();
        assert!(surface.auth.is_none());
    }
}
