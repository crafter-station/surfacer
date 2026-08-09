//! Generates the curated SUNAT declaraciones example descriptor.
//!
//! This is a hand-curated fixture, not the output of `surfacer recon`. It
//! models the real SUNAT declaration portal (`e-plataformaunica.sunat.gob.pe`)
//! so the three auth states have a real file to demonstrate against. Every
//! auth field was verified live on 2026-08-09 while building `sunat-cli`:
//!
//! - the OAuth2 token URL, password grant, and scopes are the ones a
//!   self-registered SOL client actually receives
//! - the browser-bootstrapped capture (`idCache` on the `servletAcceso`
//!   request) was observed extracting a working token
//! - the padron lookup is genuinely public
//!
//! Run with: `cargo run -p surfacer-ir --example gen_sunat_declaraciones`

use std::path::PathBuf;
use surfacer_ir::{
    AuthMode, BrowserAcquisition, BrowserBootstrappedTokenAuth, CredentialLocation, HttpEndpoint,
    HttpMethod, HttpOperation, HttpSurface, OAuth2Auth, OAuth2Grant, OperationDescriptor,
    OperationKind, OperationTransport, Provenance, ProvenanceTechnique, RenewalStrategy,
    SecretRef, SiteDescriptor, SiteMeta, TokenCapture, TokenTtl, TokenUse,
};

fn endpoint(method: HttpMethod, path: &str, description: &str, kind: OperationKind) -> HttpEndpoint {
    HttpEndpoint {
        namespace: vec!["v1".into()],
        method,
        path: path.into(),
        description: description.into(),
        operation_kind: kind,
        sample_request_content_type: None,
        sample_response_content_type: Some("application/json".into()),
        params: Vec::new(),
    }
}

fn operation(
    command_path: &[&str],
    summary: &str,
    description: &str,
    kind: OperationKind,
    endpoint_index: usize,
    auth: Option<AuthMode>,
) -> OperationDescriptor {
    OperationDescriptor {
        command_path: command_path.iter().map(|s| s.to_string()).collect(),
        summary: summary.into(),
        description: description.into(),
        operation_kind: kind,
        transport: OperationTransport::Http(HttpOperation { endpoint_index }),
        extractor: None,
        auth,
    }
}

fn main() {
    // Surface default: OAuth2, headless. This is what SIRE, GRE and CPE use.
    // A self-registered SOL client gets a password-grant token from this URL,
    // scoped to the APIs enabled at registration.
    let oauth2_default = AuthMode::OAuth2(OAuth2Auth {
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

    // Override for F616: browser-bootstrapped token. The declaration form's API
    // wants an IdCache the portal mints only during its own browser login. The
    // client cannot request that audience, so it is captured once and replayed
    // headless. Verified live.
    let f616_browser = AuthMode::BrowserBootstrappedToken(BrowserBootstrappedTokenAuth {
        acquire: BrowserAcquisition {
            login_url: "https://e-menu.sunat.gob.pe/cl-ti-itmenu/AutenticaMenuInternet.htm".into(),
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

    let descriptor = SiteDescriptor {
        meta: SiteMeta {
            site_name: "sunat-declaraciones".into(),
            display_name: "SUNAT Declaraciones (curated)".into(),
            source_url: "https://e-plataformaunica.sunat.gob.pe".into(),
            ir_version: "0.1.0".into(),
        },
        provenance: Provenance {
            generated_at: "2026-08-09T00:00:00Z".into(),
            technique: ProvenanceTechnique::Http,
            classifier_bucket: "curated-fixture".into(),
            probe_duration_sec: 0,
        },
        operations: vec![
            // SIRE ventas: inherits the OAuth2 surface default (auth: None here
            // means "inherit", not "public").
            operation(
                &["sire", "ventas", "periodos"],
                "List SIRE ventas periods",
                "Registro de Ventas e Ingresos (RVIE). OAuth2 via the surface default.",
                OperationKind::Read,
                0,
                None,
            ),
            // F616: overrides with the browser-bootstrapped mode.
            operation(
                &["f616", "periodo"],
                "Open an F616 period",
                "Trabajador Independiente monthly declaration. Needs a browser-captured IdCache.",
                OperationKind::Read,
                1,
                Some(f616_browser),
            ),
            // Padron RUC lookup: genuinely public, overrides the authed default
            // with an explicit None.
            operation(
                &["padron", "ruc"],
                "Look up a RUC in the public padron",
                "Public taxpayer lookup. No credentials required.",
                OperationKind::Read,
                2,
                Some(AuthMode::None),
            ),
        ],
        http: Some(HttpSurface {
            endpoints: vec![
                endpoint(
                    HttpMethod::Get,
                    "/v1/contribuyente/migeigv/libros/rvie/periodos",
                    "SIRE RVIE periods",
                    OperationKind::Read,
                ),
                endpoint(
                    HttpMethod::Get,
                    "/v1/recaudacion/tributaria/declaracion/pagoelectronico/trabajadorindependiente/e/obtenerPeriodo/032026",
                    "Open an F616 period (path carries the period, MMYYYY)",
                    OperationKind::Read,
                ),
                endpoint(
                    HttpMethod::Get,
                    "/v1/contribuyente/contribuyentes/10712392563",
                    "Public RUC lookup (path carries the RUC)",
                    OperationKind::Read,
                ),
            ],
            auth: Some(oauth2_default),
        }),
        ax: None,
    };

    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/sunat-declaraciones.surfacer.json");
    surfacer_ir::write_ir(&out, &descriptor).expect("write example");
    println!("wrote {}", out.display());
}
