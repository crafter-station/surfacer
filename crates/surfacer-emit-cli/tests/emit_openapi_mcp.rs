use surfacer_ir::SiteDescriptor;

/// Mirrors `examples/news-ycombinator-com.surfacer.json` so a schema change
/// fails here rather than drifting quietly.
fn descriptor_with(ops: &[(&str, &str, &[(&str, &str)])]) -> SiteDescriptor {
    let endpoints: Vec<_> = ops
        .iter()
        .map(|(name, kind, params)| {
            serde_json::json!({
                "namespace": [*name],
                "method": "GET",
                "path": format!("/{name}"),
                "description": *name,
                "operationKind": *kind,
                "params": params.iter().map(|(p, example)| serde_json::json!({
                    "name": p, "varies": false, "example": example, "observations": 1
                })).collect::<Vec<_>>()
            })
        })
        .collect();

    let operations: Vec<_> = ops
        .iter()
        .enumerate()
        .map(|(i, (name, kind, _))| {
            serde_json::json!({
                "commandPath": [*name],
                "summary": *name,
                "description": *name,
                "operationKind": *kind,
                "transport": { "kind": "http", "endpointIndex": i }
            })
        })
        .collect();

    serde_json::from_value(serde_json::json!({
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
        "operations": operations,
        "http": { "endpoints": endpoints }
    }))
    .expect("fixture IR should match the current schema")
}

/// Two operations whose command paths collide, which SUNAT produces seven of.
fn descriptor_with_colliding_commands() -> SiteDescriptor {
    let mut descriptor = descriptor_with(&[("list", "read", &[]), ("other", "read", &[])]);
    descriptor.operations[1].command_path = vec!["list".to_string()];
    descriptor
}

#[test]
fn the_openapi_document_parses_and_describes_every_operation() {
    let out = webctl_openapi(&descriptor_with(&[
        ("list", "read", &[]),
        ("show", "read", &[("id", "42")]),
    ]));

    assert_eq!(out["openapi"], "3.1.0");
    assert_eq!(
        out["paths"].as_object().expect("paths object").len(),
        2,
        "each operation needs its own path"
    );

    let show = &out["paths"]["/show"]["get"];
    assert_eq!(show["operationId"], "show");
    let params = show["parameters"].as_array().expect("parameters");
    assert_eq!(params[0]["name"], "id");
    assert_eq!(params[0]["in"], "query");
    assert_eq!(
        params[0]["schema"]["example"], "42",
        "an observed value belongs in the schema so consumers can show it"
    );
    assert_eq!(
        params[0]["required"], false,
        "recon cannot tell an optional parameter from one that always appeared"
    );
}

#[test]
fn operation_ids_stay_unique_when_command_paths_collide() {
    let out = webctl_openapi(&descriptor_with_colliding_commands());

    let ids: Vec<String> = out["paths"]
        .as_object()
        .expect("paths")
        .values()
        .flat_map(|item| item.as_object().expect("path item").values())
        .map(|op| op["operationId"].as_str().expect("operationId").to_string())
        .collect();

    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        ids.len(),
        unique.len(),
        "duplicate operationIds make the document invalid; got {ids:?}"
    );
}

#[test]
fn the_document_omits_descriptions_that_restate_the_command() {
    let out = webctl_openapi(&descriptor_with(&[("list", "read", &[])]));

    assert!(
        out["paths"]["/list"]["get"].get("description").is_none(),
        "recon derives both from the same path, so repeating it is noise"
    );
}

#[test]
fn the_mcp_server_registers_a_tool_per_operation() {
    let out = surfacer_emit_cli::emit_mcp_server(&descriptor_with(&[
        ("list", "read", &[]),
        ("show", "read", &[("id", "42")]),
    ]));

    assert!(out.contains(r#"server.registerTool(
    "list""#));
    assert!(out.contains(r#"server.registerTool(
    "show""#));
    assert!(
        out.contains(r#"id: z.string().optional().describe("observed value: 42")"#),
        "a tool must declare its parameters so a client sees them before calling"
    );
}

#[test]
fn mcp_tool_names_stay_unique_when_command_paths_collide() {
    let out = surfacer_emit_cli::emit_mcp_server(&descriptor_with_colliding_commands());

    // registerTool throws on a duplicate name, so the server would not boot.
    let count = out.matches(r#"    "list",
"#).count();
    assert_eq!(
        count, 1,
        "the second colliding operation must get a distinct tool name"
    );
    assert!(out.contains(r#""list 2""#) || out.contains(r#""list_2""#));
}

#[test]
fn writes_are_gated_in_the_mcp_server() {
    let out = surfacer_emit_cli::emit_mcp_server(&descriptor_with(&[
        ("list", "read", &[]),
        ("create", "write", &[]),
    ]));

    assert!(out.contains(r#"const ALLOWED_KINDS = new Set(["read"]);"#));
    assert!(out.contains(r#"call("create", "write""#));
    assert!(
        out.contains("annotations: { readOnlyHint: false }"),
        "a write must not advertise itself as read-only"
    );
}

fn webctl_openapi(descriptor: &SiteDescriptor) -> serde_json::Value {
    serde_json::from_str(&surfacer_emit_cli::emit_openapi(descriptor))
        .expect("the emitter must produce parseable JSON")
}
