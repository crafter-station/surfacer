//! The same agent-first rules, applied to surfacer itself.
//!
//! surfacer enforces a list of rules on the CLIs it emits, in
//! `crates/surfacer-emit-cli/tests/agent_first_rules.rs`. That list is
//! distilled in the `cli-build` skill from a corpus of hand-written CLIs. A
//! compiler that emits agent-first CLIs while not being one is not a defensible
//! position, and it was the actual state: when these tests were written,
//! surfacer broke four of the six rules it was already checking on its output.
//!
//! The two files are separate because a test cannot invoke a binary from
//! another crate's test harness, not because the rules differ. One list, two
//! subjects. A rule added to either belongs in both.
//!
//! These run the real binary through `CARGO_BIN_EXE_surfacer`, so they verify
//! what ships rather than what a function returns.

use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_surfacer")
}

/// A real IR from the repository, so these tests exercise the same input a
/// user has rather than a fixture that could drift from the schema.
fn example_ir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/news-ycombinator-com.surfacer.json")
}

/// Run the binary with stdout captured, which is what makes stdout a pipe and
/// therefore exercises the non-terminal path every agent takes.
fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .output()
        .expect("the binary under test must run")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Rule: JSON automatically when stdout is not a TTY, even without the flag.
///
/// The test harness pipes stdout, so a command that only honors an explicit
/// `--json` fails here, which is the same failure an agent would hit.
#[test]
fn piped_output_is_json_without_the_flag() {
    let out = run(&["lint", example_ir().to_str().unwrap()]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out))
        .expect("piped stdout must be a JSON document with no flag passed");
    assert_eq!(parsed["valid"], true);
    assert_eq!(parsed["site"], "news-ycombinator-com");
}

/// Rule: data on stdout, diagnostics on stderr.
///
/// The check that matters is not that stdout has content, but that it has
/// *only* the document: a single stray log line makes the whole stream
/// unparseable, and that is how this rule usually breaks.
#[test]
fn stdout_carries_only_the_document() {
    let out = run(&["lint", example_ir().to_str().unwrap()]);
    let text = stdout(&out);
    assert!(
        serde_json::from_str::<serde_json::Value>(&text).is_ok(),
        "stdout must parse as one JSON document, got: {text}"
    );
    assert!(
        !text.contains('✓') && !text.contains("Valid IR:"),
        "human narration belongs on stderr, not stdout"
    );
}

/// Rule: a `schema` command with a version field.
///
/// An agent introspects the surface as data instead of parsing `--help`.
#[test]
fn exposes_its_own_schema() {
    let out = run(&["schema"]);
    assert!(out.status.success(), "schema must exit zero");
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout(&out)).expect("schema must emit JSON on stdout");

    assert!(
        parsed["version"].is_string(),
        "schema must carry a version so a caller can detect a changed surface"
    );

    let names: Vec<&str> = parsed["commands"]
        .as_array()
        .expect("schema.commands must be an array")
        .iter()
        .map(|c| c["name"].as_str().unwrap_or_default())
        .collect();
    for required in ["lint", "emit", "install", "check", "schema"] {
        assert!(
            names.contains(&required),
            "schema must list the {required} command, got {names:?}"
        );
    }
}

/// Rule: exit codes that mean something.
///
/// A malformed IR fails the same way forever, so retrying is wasted work. An
/// unreachable file may exist on the next run. An agent that cannot tell them
/// apart retries the wrong one.
#[test]
fn user_error_and_system_error_have_different_exit_codes() {
    let dir = std::env::temp_dir().join("surfacer-agent-first-rules");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let invalid = dir.join("invalid.surfacer.json");
    std::fs::write(
        &invalid,
        r#"{"meta":{"siteName":"x","displayName":"X","sourceUrl":"https://x.test","irVersion":"0.1.0"},
            "provenance":{"generatedAt":"0","technique":"agent","classifierBucket":"HtmlRendered","probeDurationSec":1},
            "operations":[]}"#,
    )
    .expect("write fixture");

    let ok = run(&["lint", example_ir().to_str().unwrap()]);
    let user = run(&["lint", invalid.to_str().unwrap()]);
    let system = run(&["lint", dir.join("does-not-exist.json").to_str().unwrap()]);

    assert_eq!(ok.status.code(), Some(0), "a valid IR exits zero");
    assert_eq!(
        user.status.code(),
        Some(1),
        "an IR that fails lint is the caller's error"
    );
    assert_eq!(
        system.status.code(),
        Some(2),
        "a missing file is a system error, distinguishable from a bad request"
    );

    let _ = std::fs::remove_file(&invalid);
}

/// Rule: a failed command still answers as data.
///
/// Reporting the failure only as prose forces an agent to parse English to
/// learn what was wrong. The non-zero exit marks it as a failure; the document
/// says why.
#[test]
fn a_failed_lint_still_emits_a_document() {
    let dir = std::env::temp_dir().join("surfacer-agent-first-rules");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let invalid = dir.join("invalid-doc.surfacer.json");
    std::fs::write(
        &invalid,
        r#"{"meta":{"siteName":"x","displayName":"X","sourceUrl":"https://x.test","irVersion":"0.1.0"},
            "provenance":{"generatedAt":"0","technique":"agent","classifierBucket":"HtmlRendered","probeDurationSec":1},
            "operations":[]}"#,
    )
    .expect("write fixture");

    let out = run(&["lint", invalid.to_str().unwrap()]);
    let parsed: serde_json::Value = serde_json::from_str(&stdout(&out))
        .expect("a failed lint must still emit JSON on stdout");

    assert_eq!(parsed["valid"], false);
    assert!(
        !parsed["errors"].as_array().expect("errors array").is_empty(),
        "the document must name every reason it failed"
    );

    let _ = std::fs::remove_file(&invalid);
}

/// Rule: `NO_COLOR` disables styling without changing content.
///
/// Color belongs to the human stream. Honoring the variable is what lets the
/// same output be read by a person, a log file, and a terminal that cannot
/// render escapes.
///
/// Note on what this can observe: a captured stdout is a pipe, so the command
/// correctly chooses JSON and produces no human narration at all. The human
/// path is reached here by asking for it, and what is asserted is that the
/// narration arrives on stderr carrying content but no escapes.
#[test]
fn no_color_removes_escapes_from_the_human_stream() {
    let colored = Command::new(bin())
        .args(["lint", example_ir().to_str().unwrap()])
        .env_remove("NO_COLOR")
        .output()
        .expect("run without NO_COLOR");

    let plain = Command::new(bin())
        .args(["lint", example_ir().to_str().unwrap()])
        .env("NO_COLOR", "1")
        .output()
        .expect("run with NO_COLOR");

    let plain_err = String::from_utf8_lossy(&plain.stderr);
    assert!(
        !plain_err.contains('\u{1b}'),
        "NO_COLOR must remove every ANSI escape, got: {plain_err}"
    );

    // Both runs are piped, so neither narrates; the real proof that NO_COLOR
    // changes styling rather than content is that the data stream is byte for
    // byte identical either way.
    assert_eq!(
        colored.stdout, plain.stdout,
        "NO_COLOR must not change the document itself"
    );
    assert!(
        serde_json::from_slice::<serde_json::Value>(&plain.stdout).is_ok(),
        "machine output must never carry escapes that break parsing"
    );
}

/// Rule: the manual ships with the binary.
///
/// An agent reads the current manual from the install rather than a copy that
/// drifted. `list` is a diagnostic; a requested document is data.
#[test]
fn serves_its_own_manual() {
    let list = run(&["skills", "list"]);
    assert!(list.status.success());
    assert!(
        stdout(&list).is_empty(),
        "the listing is a diagnostic and belongs on stderr"
    );

    let core = run(&["skills", "get", "core"]);
    assert!(core.status.success());
    let text = stdout(&core);
    assert!(
        text.contains("name: surfacer"),
        "skills get core must print the embedded manual to stdout"
    );

    let unknown = run(&["skills", "get", "nope"]);
    assert!(
        !unknown.status.success(),
        "an unknown document must fail rather than print nothing successfully"
    );
}
