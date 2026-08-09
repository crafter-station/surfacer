//! AE-V4: the ts-cli emitter consumes auth per `docs/shaping-auth-ir.md`
//! ("How each emitter consumes auth" -> ts-cli and mcp) and the AE-V4 slice in
//! `docs/shaping-auth-emitters.md`. Three modes (apiKey / oAuth2 /
//! browserBootstrappedToken), token attached per `TokenUse`, and Mode B reads
//! the AE-V1 captured token from a path baked in at emit time.

use serde_json::{json, Value};
use surfacer_ir::SiteDescriptor;

/// One-operation descriptor with a surface-level `auth`, as raw IR JSON so the
/// fixture exercises the same deserialization a real descriptor would. Mirrors
/// the builder in `emit_openapi_auth.rs`.
fn descriptor_with_auth(site: &str, surface_auth: Value) -> SiteDescriptor {
    let mut surface = json!({
        "endpoints": [{
            "namespace": ["list"],
            "method": "GET",
            "path": "/list",
            "description": "list",
            "operationKind": "read",
            "params": []
        }]
    });
    if !surface_auth.is_null() {
        surface
            .as_object_mut()
            .unwrap()
            .insert("auth".into(), surface_auth);
    }

    serde_json::from_value(json!({
        "meta": {
            "siteName": site,
            "displayName": "Example",
            "sourceUrl": "https://example.com/start",
            "irVersion": "0.1.0"
        },
        "provenance": {
            "generatedAt": "0",
            "technique": "http",
            "classifierBucket": "HttpOnly",
            "probeDurationSec": 0
        },
        "operations": [{
            "commandPath": ["list"],
            "summary": "list",
            "description": "list",
            "operationKind": "read",
            "transport": { "kind": "http", "endpointIndex": 0 }
        }],
        "http": surface
    }))
    .expect("fixture IR should match the current schema")
}

fn browser_token() -> Value {
    json!({
        "kind": "browserBootstrappedToken",
        "acquire": {
            "loginUrl": "https://e-menu.sunat.gob.pe/cl-ti-itmenu/AutenticaMenuInternet.htm",
            "capture": { "source": "requestQueryParam", "urlContains": "servletAcceso", "param": "idCache" },
            "sessionRef": "sunat"
        },
        "use": { "location": "header", "name": "IdCache" },
        "ttl": { "seconds": 3600, "onExpiry": "promptReauth" }
    })
}

fn oauth2_password() -> Value {
    json!({
        "kind": "oAuth2",
        "grant": "password",
        "tokenUrl": "https://api-seguridad.sunat.gob.pe/v1/clientessol/x/oauth2/token/",
        "scopes": ["sire", "gre"],
        "credentials": { "from": "env", "var": "SUNAT_SOL_CREDENTIALS" },
        "ttl": { "seconds": 3600, "onExpiry": "reacquire" }
    })
}

fn api_key() -> Value {
    json!({
        "kind": "apiKey",
        "location": "header",
        "name": "X-Api-Key",
        "valuePrefix": "Bearer ",
        "secretRef": { "from": "env", "var": "SUNAT_API_KEY" }
    })
}

// (a) A browser-token descriptor emits the baked token path, the resolveToken
// helper, and the site so the standalone binary can find the AE-V1 file with no
// descriptor at hand.
#[test]
fn browser_token_bakes_the_captured_token_path_and_resolver() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("sunat", browser_token()));

    assert!(
        out.contains(r#"auth: { kind: "browserBootstrappedToken""#),
        "the resolved Mode B auth must be inlined on the operation record"
    );

    // The path is baked, not computed from a Rust helper the binary lacks.
    assert!(
        out.contains(r#"join(homedir(), ".surfacer", "sites", site, "token.json")"#),
        "the captured-token path must be baked with homedir + .surfacer/sites/<site>/token.json"
    );

    // The site is inlined so tokenFilePath(auth.site) resolves without a descriptor.
    assert!(
        out.contains(r#"site: "sunat""#),
        "the site name must be inlined on the Mode B auth record"
    );

    assert!(
        out.contains("async function resolveToken("),
        "the three-mode resolver must be emitted"
    );

    // Mode B never opens a browser headless; it prints the reauth hint and exits.
    assert!(
        out.contains(r#"run `surfacer auth login " + auth.site + "` then retry"#),
        "an expired/absent Mode B token must print the auth login hint"
    );

    // The cross-language field contract with token_cache.rs: token + expiresAt.
    assert!(
        out.contains("expiresAt: number"),
        "the captured-token shape must match {{token, expiresAt, capturedAt}}"
    );

    // The token is attached per TokenUse before the fetch.
    assert!(
        out.contains("const attached = attachToken(baseUrl, resolved);"),
        "the resolved token must be attached before the fetch"
    );
    assert!(
        out.contains(r#"await fetch(attached.url, { headers: attached.headers })"#),
        "the fetch must carry the attached headers/url, not the bare url"
    );
}

// (b) A public descriptor (no auth) emits no auth machinery on the operation:
// every op resolves to { kind: "none" } and no token path is baked.
#[test]
fn a_public_descriptor_emits_no_auth() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("example-com", Value::Null));

    assert!(
        out.contains(r#"auth: { kind: "none" }"#),
        "a public op must resolve to the none auth record"
    );
    // The Auth *type* union naturally names every mode; the gate is that no
    // operation record carries a non-none mode. Operation records are the
    // `auth: { kind: "<mode>" ...` literals emitted into OPERATIONS.
    assert!(
        !out.contains(r#"auth: { kind: "browserBootstrappedToken""#),
        "no Mode B record on a public descriptor's operations"
    );
    assert!(
        !out.contains(r#"auth: { kind: "apiKey""#) && !out.contains(r#"auth: { kind: "oAuth2""#),
        "no apiKey/oAuth2 record on a public descriptor's operations"
    );
    // resolveToken returns undefined for none, so no token is ever attached and
    // the request goes out exactly as a public one would.
    assert!(
        out.contains(r#"if (auth.kind === "none") return undefined;"#),
        "the resolver short-circuits public ops"
    );
}

// apiKey and oAuth2 records carry their runtime inputs (secret source, token
// URL, grant, use) without ever inlining a secret value (AU-R8).
#[test]
fn api_key_and_oauth2_records_name_sources_never_secrets() {
    let key = surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("k", api_key()));
    assert!(key.contains(r#"kind: "apiKey""#));
    assert!(
        key.contains(r#"secretRef: { from: "env", var: "SUNAT_API_KEY" }"#),
        "apiKey names the env var, never the value"
    );
    assert!(
        key.contains(r#"valuePrefix: "Bearer ""#),
        "the value prefix rides the record"
    );

    let oauth = surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("o", oauth2_password()));
    assert!(oauth.contains(r#"kind: "oAuth2""#));
    assert!(oauth.contains(r#"grant: "password""#));
    assert!(oauth.contains(
        r#"tokenUrl: "https://api-seguridad.sunat.gob.pe/v1/clientessol/x/oauth2/token/""#
    ));
    assert!(
        oauth.contains(r#"credentials: { from: "env", var: "SUNAT_SOL_CREDENTIALS" }"#),
        "oauth2 names the credential source, never the value"
    );
    // OAuth2 with no explicit token_use defaults to Authorization: Bearer.
    assert!(
        oauth.contains(r#"use: { location: "header", name: "Authorization", valuePrefix: "Bearer " }"#),
        "oauth2 defaults token use to Authorization: Bearer"
    );
    assert!(
        oauth.contains("reacquire: true"),
        "an onExpiry: reacquire oauth2 token records reacquire = true"
    );
}

// The write-gate is untouched by auth: an authed surface still gates writes by
// kind, exactly as before.
#[test]
fn auth_does_not_touch_the_write_gate() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("sunat", browser_token()));
    assert!(
        out.contains(r#"const ALLOWED_KINDS = new Set<string>(["read"]);"#),
        "the write-gate stays orthogonal to auth"
    );
}

// (c) The generated TypeScript is syntactically valid. Uses `bun build` as the
// gate when bun is on PATH (it parses/transpiles and fails on a syntax error);
// skips with a printed note where bun is absent so CI without bun does not
// false-fail. This follows the repo convention of not hard-depending on a JS
// toolchain in the emit tests while still proving validity where possible.
#[test]
fn the_generated_typescript_is_syntactically_valid() {
    use std::io::Write;
    use std::process::Command;

    // Cover all three auth modes plus a public op in one emitted program.
    let sources = [
        surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("sunat", browser_token())),
        surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("o", oauth2_password())),
        surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("k", api_key())),
        surfacer_emit_cli::emit_ts_cli(&descriptor_with_auth("pub", Value::Null)),
    ];

    let bun = Command::new("bun").arg("--version").output();
    if bun.is_err() {
        eprintln!("skipping bun syntax gate: bun not on PATH");
        return;
    }

    let dir = std::env::temp_dir();
    for (i, src) in sources.iter().enumerate() {
        let path = dir.join(format!("surfacer_ts_cli_auth_{i}.ts"));
        let mut f = std::fs::File::create(&path).expect("write temp ts");
        f.write_all(src.as_bytes()).expect("write ts source");

        let out = Command::new("bun")
            .arg("build")
            .arg("--target=node")
            .arg(&path)
            .output()
            .expect("run bun build");

        assert!(
            out.status.success(),
            "bun build rejected the emitted TS ({}):\nstdout: {}\nstderr: {}",
            path.display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );

        let _ = std::fs::remove_file(&path);
    }
}
