use surfacer_ir::SiteDescriptor;

/// Mirrors `examples/news-ycombinator-com.surfacer.json` so a schema change fails
/// here rather than drifting quietly.
fn descriptor_with(ops: &[(&str, &str, &[(&str, &str)])]) -> SiteDescriptor {
    let endpoints: Vec<_> = ops
        .iter()
        .map(|(name, kind, params)| {
            let query: Vec<String> = params.iter().map(|(p, _)| format!("{p}={{{p}}}")).collect();
            let path = if query.is_empty() {
                format!("/{name}")
            } else {
                format!("/{name}?{}", query.join("&"))
            };
            serde_json::json!({
                "namespace": [*name],
                "method": "GET",
                "path": path,
                "description": *name,
                "operationKind": *kind,
                "params": params.iter().map(|(p, example)| serde_json::json!({
                    "name": p,
                    "varies": false,
                    "example": example,
                    "observations": 1
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

#[test]
fn emits_a_program_scriptc_can_compile() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with(&[("list", "read", &[])]));

    assert!(
        out.contains("async function main()"),
        "scriptc rejects top-level await, so the entry point must be a function"
    );
    assert!(
        !out.contains("\nconst code = await "),
        "no top-level await may survive into the emitted program"
    );
    assert!(
        out.contains("--dynamic"),
        "the build hint must tell the user fetch needs the embedded engine"
    );
}

#[test]
fn the_generated_cli_carries_the_whole_descriptor() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with(&[
        ("list", "read", &[]),
        ("show", "read", &[("id", "42")]),
    ]));

    assert!(out.contains(r#"name: "list""#), "operations must be inlined");
    assert!(out.contains(r#"name: "show""#), "operations must be inlined");
    // The Rust shim spawns `surfacer exec`; this self-contained program must
    // not. It may still *name* the surfacer CLI: AE-V4 bakes the token path
    // `~/.surfacer/sites/<site>/token.json` and prints a `surfacer auth login`
    // reauth hint, both legitimate. The gate is on shelling out, not the word.
    let code = out
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for spawn in [
        "spawn(\"surfacer",
        "exec(\"surfacer",
        "execSync(\"surfacer",
        "spawnSync(\"surfacer",
        "child_process",
    ] {
        assert!(
            !code.contains(spawn),
            "the emitted CLI must not shell out to surfacer ({spawn}); the Rust shim does that, this one is self-contained"
        );
    }
}

#[test]
fn params_reach_both_help_and_the_request() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with(&[("user", "read", &[("id", "Hunter17")])]));

    assert!(
        out.contains(r#"name: "id""#) && out.contains(r#"example: "Hunter17""#),
        "observed params must be inlined with their example"
    );
    assert!(
        out.contains("observations:") && out.contains("varies:"),
        "the evidence behind a param must travel with it, since the emitted \
         program refuses a call that omits one every observation carried"
    );
    assert!(
        out.contains("withQuery("),
        "params must reach the request URL"
    );
}

#[test]
fn the_base_url_drops_the_recon_time_query_template() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with(&[("user", "read", &[("id", "Hunter17")])]));

    assert!(
        out.contains(r#"path: "/user""#),
        "only the path portion belongs in the request; params are supplied at call time"
    );
    assert!(
        !out.contains("{id}"),
        "the recon-time query template must not leak into the emitted program"
    );
}

#[test]
fn writes_are_gated_by_default() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with(&[
        ("list", "read", &[]),
        ("create", "write", &[]),
    ]));

    assert!(
        out.contains(r#"const ALLOWED_KINDS = new Set<string>(["read"]);"#),
        "recon cannot tell a harmless write from a destructive one, so writes stay blocked"
    );
    assert!(out.contains(r#"kind: "write""#), "the kind must survive into the program");
}

#[test]
fn quotes_in_descriptions_do_not_break_the_emitted_program() {
    let mut descriptor = descriptor_with(&[("list", "read", &[])]);
    descriptor.operations[0].description = "say \"hi\"\nand newline".to_string();
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor);

    assert!(
        out.contains(r#"description: "say \"hi\" and newline""#),
        "quotes must be escaped and newlines flattened"
    );
}

#[test]
fn the_program_drops_descriptions_that_restate_the_command() {
    let out = surfacer_emit_cli::emit_ts_cli(&descriptor_with(&[("list", "read", &[])]));

    assert!(
        out.contains("function addsNothing("),
        "recon derives description and command from the same path, so the \
         emitted program must decide at runtime whether to print both"
    );
    assert!(
        out.contains("addsNothing(op.name, op.description)"),
        "the help loop must consult it"
    );
}
