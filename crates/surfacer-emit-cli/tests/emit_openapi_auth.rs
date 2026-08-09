//! AE-V3: the openapi emitter carries auth per `docs/shaping-auth-ir.md`
//! ("How each emitter consumes auth" -> openapi) and D3 (Mode B has no OpenAPI
//! scheme, so it degrades to an `x-surfacer-auth` extension, never a faked one).

use serde_json::{json, Value};
use surfacer_ir::SiteDescriptor;

/// Build a one-operation descriptor with a given surface-level `auth` and an
/// optional per-operation `auth` override, both as raw IR JSON so the fixture
/// exercises the same deserialization a real descriptor would.
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
            "siteName": "example-com",
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
        "operations": [operation],
        "http": surface
    }))
    .expect("fixture IR should match the current schema")
}

fn emit(descriptor: &SiteDescriptor) -> Value {
    // (c) the emitted spec must always stay parseable JSON.
    serde_json::from_str(&surfacer_emit_cli::emit_openapi(descriptor))
        .expect("the emitter must produce parseable JSON")
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

// (a) A descriptor with oauth2 produces a securityScheme oauth2 and the op
// references it.
#[test]
fn oauth2_surface_emits_a_scheme_the_operation_references() {
    let out = emit(&descriptor_with_auth(oauth2_password(), None));

    let schemes = out["components"]["securitySchemes"]
        .as_object()
        .expect("an oauth2 surface must produce components.securitySchemes");
    assert_eq!(schemes.len(), 1, "one distinct scheme on this surface");

    let (name, scheme) = schemes.iter().next().unwrap();
    assert_eq!(scheme["type"], "oauth2", "the scheme is oauth2");
    let flow = &scheme["flows"]["password"];
    assert_eq!(
        flow["tokenUrl"], "https://api-seguridad.sunat.gob.pe/v1/clientessol/x/oauth2/token/",
        "the password flow carries the observed token URL"
    );
    assert!(
        flow["scopes"].is_object(),
        "OpenAPI requires a scopes map, even if empty"
    );
    assert_eq!(
        flow["scopes"]["sire"], "",
        "each observed scope is a map key"
    );

    // The operation must reference the scheme by its exact name.
    let security = out["paths"]["/list"]["get"]["security"]
        .as_array()
        .expect("an authenticated op carries a security array");
    assert_eq!(security.len(), 1);
    assert!(
        security[0].get(name).is_some(),
        "the op references the scheme name {name:?}; got {security:?}"
    );
}

// (b) A descriptor with browser-token produces the `x-surfacer-auth` extension
// and NOT a faked scheme, and is omitted from `security` (D3).
#[test]
fn browser_token_degrades_to_extension_never_a_fake_scheme() {
    let out = emit(&descriptor_with_auth(browser_token(), None));

    // No securitySchemes at all: OpenAPI has no vocabulary for Mode B, so the
    // emitter must not invent one (the whole point of D3).
    assert!(
        out.get("components").is_none(),
        "Mode B must not fabricate a securityScheme; got {:?}",
        out.get("components")
    );

    let op = &out["paths"]["/list"]["get"];
    assert!(
        op.get("security").is_none(),
        "a browser-bootstrapped op is omitted from security, not faked"
    );

    let ext = &op["x-surfacer-auth"];
    assert_eq!(
        ext["kind"], "browserBootstrappedToken",
        "the full auth JSON rides the x-surfacer-auth extension"
    );
    assert_eq!(
        ext["use"]["name"], "IdCache",
        "extension carries the token use"
    );
    assert_eq!(
        ext["acquire"]["capture"]["param"], "idCache",
        "extension carries the capture detail"
    );

    let description = op["description"]
        .as_str()
        .expect("a note in the description");
    assert!(
        description.contains("surfacer auth login"),
        "the note tells a reader to run auth login; got {description:?}"
    );
}

// A surface where oauth2 is the default and one op opts out to None gets the
// explicit `security: []` public marker, while the authenticated sibling keeps
// its reference. Exercises resolve_auth override + the D3 "explicit public"
// branch. Also confirms (c) parseability with a mixed surface.
#[test]
fn explicit_none_override_marks_public_against_an_authenticated_default() {
    // Surface defaults to oauth2; the single op overrides to None.
    let out = emit(&descriptor_with_auth(
        oauth2_password(),
        Some(json!({ "kind": "none" })),
    ));

    let op = &out["paths"]["/list"]["get"];
    let security = op["security"]
        .as_array()
        .expect("an explicit public op against an authed default carries security: []");
    assert!(
        security.is_empty(),
        "security: [] marks the op public on purpose; got {security:?}"
    );

    // The surface default never got used by any op, so no scheme is emitted.
    // (Schemes are only registered for ops that actually reference them.)
    assert!(
        out.get("components").is_none(),
        "no op used the oauth2 default, so no scheme is emitted"
    );
}

// apiKey maps to a `type: apiKey` scheme with in/name, referenced by the op.
#[test]
fn api_key_surface_emits_an_apikey_scheme() {
    let out = emit(&descriptor_with_auth(
        json!({
            "kind": "apiKey",
            "location": "header",
            "name": "X-Api-Key",
            "secretRef": { "from": "env", "var": "SUNAT_API_KEY" }
        }),
        None,
    ));

    let schemes = out["components"]["securitySchemes"]
        .as_object()
        .expect("an apiKey surface must produce a scheme");
    let (name, scheme) = schemes.iter().next().unwrap();
    assert_eq!(scheme["type"], "apiKey");
    assert_eq!(scheme["in"], "header");
    assert_eq!(scheme["name"], "X-Api-Key");

    let security = out["paths"]["/list"]["get"]["security"]
        .as_array()
        .expect("security array");
    assert!(security[0].get(name).is_some());
}

// An unauthenticated surface is untouched: no components, no per-op security.
#[test]
fn unauthenticated_surface_is_left_exactly_as_before() {
    let out = emit(&descriptor_with_auth(Value::Null, None));
    assert!(
        out.get("components").is_none(),
        "no auth, no components block"
    );
    assert!(
        out["paths"]["/list"]["get"].get("security").is_none(),
        "no auth default means no security key on a public op"
    );
}
