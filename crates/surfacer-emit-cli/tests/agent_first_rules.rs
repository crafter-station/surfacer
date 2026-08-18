//! Agent-first rules the emitted CLI must satisfy.
//!
//! These are not style preferences. Each one is distilled in the `cli-build`
//! skill from a corpus of hand-written CLIs, where the same defect appeared
//! independently in more than one of them. The prose that explains why each
//! rule exists, and which real CLI it was learned from, lives in that skill:
//! `crafter-station/skills`, `skills/cli-build/SKILL.md`. Every test below
//! names the rule it enforces so the two can be read together.
//!
//! Codifying them here rather than leaving them as prose is deliberate. A rule
//! written only in markdown can be violated by a generator forever without
//! anyone noticing, which is what happened to the two rules covered by
//! `tty_implies_json` and `diagnostics_go_to_stderr`.
//!
//! Scope: `ts_cli` and `shim`, the two targets that emit a CLI. The `mcp`
//! target is deliberately not covered here; see the note in `src/mcp.rs`.
//!
//! **The same rules bind surfacer itself**, and they are checked against the
//! binary in `crates/surfacer-app/tests/agent_first_rules.rs`. The two files
//! are separate only because a test cannot invoke a binary from another
//! crate's test harness; they enforce one list against two subjects, the
//! compiler and what it compiles. A rule added here belongs there too. When
//! that second file was written, surfacer broke four of the six rules it was
//! already enforcing on its own output.

use surfacer_ir::SiteDescriptor;

/// A descriptor with one read operation and one write operation, which is the
/// smallest input that exercises both the always-on rules and the ones that
/// only apply when an operation can change something.
fn descriptor() -> SiteDescriptor {
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
        "operations": [
            {
                "commandPath": ["item", "list"],
                "summary": "List items",
                "description": "List items",
                "operationKind": "read",
                "transport": { "kind": "http", "endpointIndex": 0 }
            },
            {
                "commandPath": ["item", "create"],
                "summary": "Create an item",
                "description": "Create an item",
                "operationKind": "write",
                "transport": { "kind": "http", "endpointIndex": 1 }
            }
        ],
        "http": {
            "endpoints": [
                {
                    "namespace": ["item"],
                    "method": "GET",
                    "path": "/items",
                    "description": "List items",
                    "operationKind": "read",
                    "params": []
                },
                {
                    "namespace": ["item"],
                    "method": "POST",
                    "path": "/items",
                    "description": "Create an item",
                    "operationKind": "write",
                    "params": []
                }
            ]
        }
    }))
    .expect("descriptor fixture must match the current IR schema")
}

fn emit_ts() -> String {
    surfacer_emit_cli::emit_ts_cli(&descriptor())
}

/// Rule: JSON automatically when stdout is not a TTY, even without the flag.
///
/// An agent invoking the CLI through a pipe gets machine output without
/// knowing to ask for it. The corpus converged on this independently in three
/// CLIs, which is the strongest signal in the whole set.
#[test]
fn tty_implies_json() {
    let out = emit_ts();
    assert!(
        out.contains("isTTY"),
        "emitted CLI must consult process.stdout.isTTY so a piped invocation \
         gets JSON without the flag (cli-build: agent-first defaults)"
    );
}

/// Rule: data on stdout, diagnostics on stderr, always.
///
/// Help text, banners, and progress are diagnostics. When they share stdout
/// with data, piping the CLI into a parser breaks in a way that looks like a
/// malformed response rather than a misdirected stream.
#[test]
fn diagnostics_go_to_stderr() {
    let out = emit_ts();
    let help_on_stdout = out.contains("console.log(buildHelp())");
    assert!(
        !help_on_stdout,
        "help text is a diagnostic and must go to stderr, not stdout \
         (cli-build: data on stdout, diagnostics on stderr)"
    );
}

/// Rule: a `schema` command with a version field.
///
/// Agents introspect at runtime instead of parsing `--help`. Present in only
/// 3 of 14 corpus CLIs: the highest-value convention and the least adopted.
#[test]
fn exposes_schema_command() {
    let out = emit_ts();
    assert!(
        out.contains("schema"),
        "emitted CLI must expose a schema command for runtime introspection"
    );
}

/// Rule: `--json` exists as an explicit output-mode flag.
///
/// It means output mode, never input. Two corpus CLIs overloaded it as an
/// input flag and broke the convention agents expect.
#[test]
fn exposes_json_flag() {
    let out = emit_ts();
    assert!(
        out.contains("--json"),
        "emitted CLI must accept --json as an output-mode flag"
    );
}

/// Rule: exit codes that mean something.
///
/// Zero is success; a user error and a system failure are distinguishable.
/// An agent that cannot tell them apart retries the ones it should not.
#[test]
fn uses_meaningful_exit_codes() {
    let out = emit_ts();
    assert!(
        out.contains("process.exit"),
        "emitted CLI must exit non-zero on failure"
    );
}

/// Rule: a call the target was never observed answering fails before the
/// request, not after.
///
/// Without this the target replies 200 with its own error page ("No such
/// user."), the command exits zero, and the caller reads that as success. The
/// IR records which parameters every observed request carried, so the emitted
/// program has what it needs to refuse first.
#[test]
fn a_call_missing_an_observed_parameter_fails_before_the_request() {
    let out = emit_ts();
    assert!(
        out.contains("missing parameter"),
        "emitted CLI must reject a call that omits a parameter every observed \
         request carried, rather than sending it and reporting the target's \
         error page as success"
    );
    assert!(
        out.contains("observations"),
        "the check must be driven by observation counts from the IR, so it \
         claims evidence rather than a contract the target never published"
    );
}

/// Rule: no prompt ever blocks a non-interactive run.
///
/// Anything that would prompt fails with a structured error instead of
/// hanging. A hung agent looks like a slow one until a timeout fires.
#[test]
fn never_prompts_without_a_tty() {
    let out = emit_ts();
    let prompts = out.contains("readline") || out.contains("createInterface");
    if prompts {
        assert!(
            out.contains("isTTY"),
            "a CLI that can prompt must check for a TTY first and fail with a \
             structured error when there is none (cli-build: no prompt blocks \
             a non-interactive run)"
        );
    }
}
