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

// --- AE-V6: auth in shell -------------------------------------------------

/// The curated SUNAT descriptor carries all three interesting auth states:
/// a browser-bootstrapped F616 op, a public padron lookup, and an OAuth2
/// surface default. Loading it keeps these tests honest against the real IR.
fn sunat_descriptor() -> SiteDescriptor {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/sunat-declaraciones.surfacer.json"
    );
    let json = std::fs::read_to_string(path).expect("SUNAT fixture must exist");
    serde_json::from_str(&json).expect("SUNAT fixture must match the current schema")
}

#[test]
fn browser_token_op_reads_the_baked_token_json_and_attaches_the_header() {
    let out = surfacer_emit_cli::emit_executor_config(&sunat_descriptor());

    // The token comes from the file `auth login` baked, at the exact path the
    // cross-language contract fixes.
    assert!(
        out.contains(r#"$HOME/.surfacer/sites/sunat-declaraciones/token.json"#),
        "Mode B must read the baked token.json path:\n{out}"
    );
    assert!(
        out.contains("jq -r .token <"),
        "Mode B must read the token with jq:\n{out}"
    );
    // The IdCache header rides the real curl, with the resolved token.
    assert!(
        out.contains(r#"curl -sS -H "IdCache: ${token}" "$url""#),
        "Mode B must attach IdCache on the curl:\n{out}"
    );
}

#[test]
fn browser_token_op_expiry_checks_and_prints_the_reauth_hint() {
    let out = surfacer_emit_cli::emit_executor_config(&sunat_descriptor());

    // Expiry check against .expiresAt, in shell.
    assert!(
        out.contains("jq -r .expiresAt <"),
        "Mode B must read the expiry:\n{out}"
    );
    assert!(
        out.contains(r#"[ "$expires_at" -le $((now + 60)) ]"#),
        "Mode B must compare .expiresAt against now with a skew buffer:\n{out}"
    );
    // Missing or stale prints the exact reauth hint and returns non-zero.
    assert!(
        out.contains(r#"run: surfacer auth login sunat-declaraciones"#),
        "Mode B must print the reauth hint with the site name:\n{out}"
    );
    assert!(
        out.contains("return 1"),
        "Mode B must return non-zero when the token is missing or stale:\n{out}"
    );
}

#[test]
fn oauth2_surface_op_posts_the_token_url_and_pipes_through_jq() {
    let out = surfacer_emit_cli::emit_executor_config(&sunat_descriptor());

    // The SIRE op inherits the OAuth2 surface default: POST the token URL, then
    // jq out the access_token.
    assert!(
        out.contains(r#"curl -sS -X POST "https://api-seguridad.sunat.gob.pe"#),
        "OAuth2 must POST the token URL:\n{out}"
    );
    assert!(
        out.contains(r#"jq -r '.access_token'"#),
        "OAuth2 must extract access_token with jq:\n{out}"
    );
    assert!(
        out.contains(r#"--data-urlencode "grant_type=password""#),
        "OAuth2 must send the observed grant type:\n{out}"
    );
    // Default TokenUse: Authorization: Bearer.
    assert!(
        out.contains(r#"curl -sS -H "Authorization: Bearer ${token}" "$url""#),
        "OAuth2 must attach the bearer token on the real curl:\n{out}"
    );
}

#[test]
fn public_operation_emits_no_auth() {
    // A descriptor with no auth anywhere must not grow a shell auth block.
    let out = surfacer_emit_cli::emit_executor_config(&descriptor_with(&[("list", "read")]));

    assert!(
        !out.contains("surfacerShellAuth"),
        "a public descriptor must not export a shell auth block:\n{out}"
    );
    assert!(
        !out.contains("curl"),
        "a public descriptor must keep the direct fetch, no curl:\n{out}"
    );
    assert!(
        out.contains("const res = await fetch(url);"),
        "public ops keep the direct fetch:\n{out}"
    );
    // The explicit `none` op in SUNAT is likewise auth-free.
    let sunat = surfacer_emit_cli::emit_executor_config(&sunat_descriptor());
    assert!(
        !sunat.contains("__surfacer_auth_sunat_declaraciones_padron_ruc"),
        "an explicit `none` op must not get a shell auth function:\n{sunat}"
    );
}

/// Extract the `surfacerShellAuth` template-literal body and run `bash -n` over
/// it. This proves the generated shell parses, not just that the strings are
/// present. Skips cleanly if `bash` is unavailable.
#[test]
fn generated_shell_auth_block_passes_bash_syntax_check() {
    let out = surfacer_emit_cli::emit_executor_config(&sunat_descriptor());

    let marker = "export const surfacerShellAuth = String.raw`";
    let start = out
        .find(marker)
        .map(|i| i + marker.len())
        .expect("SUNAT descriptor must emit a shell auth block");
    let rest = &out[start..];
    let end = rest.find('`').expect("shell auth block must be closed");
    let shell = &rest[..end];

    // Sanity: the block is non-trivial and carries all three shell tools.
    assert!(shell.contains("curl"));
    assert!(shell.contains("jq"));
    assert!(shell.contains("qsep()"));

    let output = std::process::Command::new("bash")
        .arg("-n")
        .arg("/dev/stdin")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("stdin")
                .write_all(shell.as_bytes())?;
            child.wait_with_output()
        });

    match output {
        Ok(result) => assert!(
            result.status.success(),
            "generated shell must pass `bash -n`:\n{}\n--- shell ---\n{shell}",
            String::from_utf8_lossy(&result.stderr)
        ),
        Err(e) => eprintln!("skipping bash syntax check, bash unavailable: {e}"),
    }
}
