//! AE-V5: the mcp emitter carries auth per `docs/shaping-auth-ir.md`
//! ("How each emitter consumes auth" -> ts-cli and mcp). The MCP shape has no
//! process exit, so a missing or expired browser-bootstrapped token becomes a
//! tool error carrying the `surfacer auth login <site>` hint, never an exit and
//! never a browser attempt inside the server.

use serde_json::{json, Value};
use surfacer_ir::SiteDescriptor;

/// Build a one-operation descriptor with a given surface-level `auth` and an
/// optional per-operation `auth` override, as raw IR JSON so the fixture
/// exercises the same deserialization a real descriptor would. Mirrors the
/// AE-V3 openapi auth test fixture.
fn descriptor_with_auth(surface_auth: Value, op_auth: Option<Value>) -> SiteDescriptor {
    let mut operation = json!({
        "commandPath": ["list"],
        "summary": "list",
        "description": "list",
        "operationKind": "read",
        "transport": { "kind": "http", "endpointIndex": 0 }
    });
    if let Some(auth) = op_auth {
        operation
            .as_object_mut()
            .unwrap()
            .insert("auth".into(), auth);
    }

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
            "siteName": "sunat",
            "displayName": "SUNAT",
            "sourceUrl": "https://e-plataformaunica.sunat.gob.pe/start",
            "irVersion": "0.1.0"
        },
        "provenance": {
            "generatedAt": "0",
            "technique": "http",
            "classifierBucket": "HttpOnly",
            "probeDurationSec": 0
        },
        "operations": [operation],
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

// (a) A browser-token op bakes the token.json path per the cross-language
// contract and turns the missing/expired case into a tool error with the
// reauth hint, never a process exit.
#[test]
fn browser_token_bakes_path_and_surfaces_reauth_as_a_tool_error() {
    let js = surfacer_emit_cli::emit_mcp_server(&descriptor_with_auth(browser_token(), None));

    // The path is baked from homedir() so it survives a different HOME than
    // emit time, and it follows $HOME/.surfacer/sites/<site>/token.json.
    assert!(
        js.contains(r#"join(homedir(), ".surfacer", "sites", "sunat", "token.json")"#),
        "the token.json path must be baked from homedir() for site sunat; got:\n{js}"
    );
    assert!(
        js.contains(r#"import { homedir } from "node:os";"#)
            && js.contains(r#"import { readFileSync } from "node:fs";"#),
        "the runtime must import homedir and readFileSync to read the baked path"
    );

    // Reauth is a tool error (isError: true via toolError), not an exit. The
    // message names the exact command and the site.
    assert!(
        js.contains("surfacer auth login sunat"),
        "the reauth hint must name `surfacer auth login sunat`; got:\n{js}"
    );
    assert!(
        js.contains("function toolError(text)")
            && js.contains("isError: true"),
        "a missing/expired token must return a tool error (isError), not exit"
    );
    // A headless server must never attempt the browser flow.
    assert!(
        !js.contains("agent-browser") && !js.contains("process.exit"),
        "the emitted MCP server must not open a browser or exit the process"
    );

    // The op still carries an AUTH entry keyed by its tool name, and the token
    // is attached as the IdCache header per TokenUse.
    assert!(
        js.contains(r#"mode: "browserToken""#) && js.contains(r#"name: "IdCache""#),
        "the AUTH record must model the browser token and its IdCache header use"
    );
}

// (b) A public descriptor emits no auth: an empty AUTH map, no baked token
// path, and no token attach. This is the "auth never widens surface" guarantee
// at the codegen level.
#[test]
fn public_descriptor_emits_no_auth() {
    let js = surfacer_emit_cli::emit_mcp_server(&descriptor_with_auth(
        json!({ "kind": "none" }),
        None,
    ));

    assert!(
        js.contains("const AUTH = {};"),
        "a public descriptor must emit an empty AUTH map; got:\n{js}"
    );
    assert!(
        !js.contains("token.json") && !js.contains(r#"mode: "browserToken""#),
        "no auth means no baked token path and no auth record"
    );

    // A descriptor with no auth field at all (legacy, resolve_auth -> None)
    // behaves identically.
    let legacy = surfacer_emit_cli::emit_mcp_server(&descriptor_with_auth(Value::Null, None));
    assert!(
        legacy.contains("const AUTH = {};"),
        "a legacy descriptor with no auth field also emits an empty AUTH map"
    );
}

// (c) The generated JS is syntactically coherent: braces/brackets/parens
// balance, and the structural markers the runtime needs are present and well
// formed. (A node --check pass is run manually as observed evidence; this keeps
// the guarantee in-tree without a node dependency in CI.)
#[test]
fn generated_js_is_syntactically_coherent() {
    // Exercise two auth modes at once: op 0 inherits the oauth2 surface
    // default, op 1 overrides to browser-token. Both record shapes plus every
    // helper render together in one file.
    let descriptor: SiteDescriptor = serde_json::from_value(json!({
        "meta": {
            "siteName": "sunat",
            "displayName": "SUNAT",
            "sourceUrl": "https://e-plataformaunica.sunat.gob.pe/start",
            "irVersion": "0.1.0"
        },
        "provenance": {
            "generatedAt": "0",
            "technique": "http",
            "classifierBucket": "HttpOnly",
            "probeDurationSec": 0
        },
        "operations": [
            {
                "commandPath": ["sire"],
                "summary": "sire",
                "description": "sire",
                "operationKind": "read",
                "transport": { "kind": "http", "endpointIndex": 0 }
            },
            {
                "commandPath": ["f616"],
                "summary": "f616",
                "description": "f616",
                "operationKind": "read",
                "transport": { "kind": "http", "endpointIndex": 1 },
                "auth": browser_token()
            }
        ],
        "http": {
            "auth": oauth2_password(),
            "endpoints": [
                { "namespace": ["sire"], "method": "GET", "path": "/sire", "description": "sire", "operationKind": "read", "params": [] },
                { "namespace": ["f616"], "method": "GET", "path": "/f616", "description": "f616", "operationKind": "read", "params": [] }
            ]
        }
    }))
    .expect("two-op fixture should match the schema");
    let js = surfacer_emit_cli::emit_mcp_server(&descriptor);

    for (open, close) in [('{', '}'), ('[', ']'), ('(', ')')] {
        let opens = js.chars().filter(|c| *c == open).count();
        let closes = js.chars().filter(|c| *c == close).count();
        assert_eq!(
            opens, closes,
            "unbalanced {open}{close}: {opens} open vs {closes} close in emitted JS"
        );
    }

    // The runtime pieces AE-V5 adds must all be present.
    for marker in [
        "async function resolveToken(auth)",
        "function attachToken(url, headers, use, token)",
        "async function resolveOAuth2(auth)",
        "function readSecret(ref)",
        "const auth = AUTH[name];",
        "url = attachToken(url, headers, auth.use, resolved.token);",
    ] {
        assert!(
            js.contains(marker),
            "emitted JS is missing runtime piece {marker:?}"
        );
    }

    // The oauth2 record renders with its token URL and default Bearer use.
    assert!(
        js.contains(r#"mode: "oAuth2""#)
            && js.contains("api-seguridad.sunat.gob.pe")
            && js.contains(r#"valuePrefix: "Bearer ""#),
        "the oauth2 record must carry its token URL and default Bearer prefix"
    );
    // The browser-token override wins over the oauth2 surface default for the op.
    assert!(
        js.contains(r#"mode: "browserToken""#),
        "the per-op browser-token override must win over the oauth2 default"
    );
}
