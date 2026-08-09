use surfacer_ir::{OperationKind, SiteDescriptor};

/// Build a descriptor with one HTTP endpoint per named operation.
///
/// Shapes mirror `examples/news-ycombinator-com.surfacer.json` so a schema change
/// fails this test instead of drifting silently.
fn descriptor_with(ops: &[(&str, &str)]) -> SiteDescriptor {
    let endpoints: Vec<_> = ops
        .iter()
        .map(|(name, kind)| {
            serde_json::json!({
                "namespace": [*name],
                "method": "GET",
                "path": format!("/{name}"),
                "description": *name,
                "operationKind": *kind
            })
        })
        .collect();

    let operations: Vec<_> = ops
        .iter()
        .enumerate()
        .map(|(i, (name, kind))| {
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
fn emits_stable_hook_wiring_not_legacy_executor_option() {
    let out = surfacer_emit_cli::emit_executor_config(&descriptor_with(&[("list", "read")]));

    assert!(
        out.contains("createSurfacerExecutor"),
        "must export the executor factory"
    );
    assert!(
        out.contains("invokeTool: executor.invokeTool"),
        "usage must wire the stable invokeTool hook"
    );
    assert!(
        !out.contains("new Bash({ executor:"),
        "must not emit the pre-PR209 `executor` option, which no longer exists"
    );
}

#[test]
fn write_operations_are_gated_read_operations_are_not() {
    let out = surfacer_emit_cli::emit_executor_config(&descriptor_with(&[
        ("list", "read"),
        ("create", "write"),
    ]));

    assert!(
        out.contains(r#"assertAllowed("example-com.list", "read")"#),
        "read operation must carry its kind into the gate"
    );
    assert!(
        out.contains(r#"assertAllowed("example-com.create", "write")"#),
        "write operation must carry its kind into the gate"
    );
    assert!(
        out.contains(r#"const ALLOWED_KINDS = new Set(["read"]);"#),
        "gate must default to read-only; inline tools bypass onToolApproval"
    );
}

#[test]
fn operations_receive_args_and_forward_them() {
    let out = surfacer_emit_cli::emit_executor_config(&descriptor_with(&[("list", "read")]));

    assert!(
        out.contains("execute: async (args) =>"),
        "execute must accept args; the pre-PR209 emitter ignored them"
    );
    assert!(
        out.contains("withQuery("),
        "args must reach the request URL"
    );
}

#[test]
fn descriptions_with_quotes_do_not_break_the_emitted_js() {
    let mut descriptor = descriptor_with(&[("list", "read")]);
    descriptor.operations[0].description = "say \"hi\"\nand newline".to_string();
    let out = surfacer_emit_cli::emit_executor_config(&descriptor);

    assert!(
        out.contains(r#"description: "say \"hi\" and newline","#),
        "quotes must be escaped and newlines flattened"
    );
}

#[test]
fn every_operation_kind_survives_the_round_trip() {
    let out = surfacer_emit_cli::emit_executor_config(&descriptor_with(&[
        ("a", "read"),
        ("b", "write"),
        ("c", "other"),
    ]));

    for (name, kind) in [("a", "read"), ("b", "write"), ("c", "other")] {
        assert!(
            out.contains(&format!(r#"assertAllowed("example-com.{name}", "{kind}")"#)),
            "{kind} operation must reach the gate with its kind intact"
        );
    }
    let _ = OperationKind::Other;
}
