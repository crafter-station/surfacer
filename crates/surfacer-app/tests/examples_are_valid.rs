//! Every committed example must pass the gate the docs tell users to run.
//!
//! `examples/` is the first thing a new user installs, and the documented flow
//! starts with `surfacer lint`. An example that fails it turns the quick start
//! into a bug report on the first command.
//!
//! This is not hypothetical. `www-sunat-gob-pe.surfacer.json` shipped with
//! seven operations collapsing onto one `operatividadaduanera novedades`
//! command path, so the file in the repository failed its own linter. Nothing
//! caught it because CI emitted from one example and linted none.

use std::path::PathBuf;
use std::process::Command;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Collect every committed descriptor, so adding one to the directory is
/// enough to put it under this gate.
fn example_irs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(examples_dir())
        .expect("examples directory must exist")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".surfacer.json"))
        .collect();
    found.sort();
    found
}

#[test]
fn every_committed_example_passes_lint() {
    let irs = example_irs();
    assert!(
        !irs.is_empty(),
        "expected at least one committed example to guard"
    );

    let mut failures = Vec::new();
    for ir in &irs {
        let out = Command::new(env!("CARGO_BIN_EXE_surfacer"))
            .args(["lint", ir.to_str().expect("path is utf-8")])
            .output()
            .expect("the binary under test must run");

        if !out.status.success() {
            let name = ir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            // Report every reason at once. Fixing these one failure per run is
            // what let seven duplicates of the same shape accumulate.
            let reasons: Vec<String> = serde_json::from_slice::<serde_json::Value>(&out.stdout)
                .ok()
                .and_then(|doc| {
                    doc["errors"].as_array().map(|errs| {
                        errs.iter()
                            .map(|e| e.as_str().unwrap_or_default().to_string())
                            .collect()
                    })
                })
                .unwrap_or_default();
            failures.push(format!("{name}: {}", reasons.join("; ")));
        }
    }

    assert!(
        failures.is_empty(),
        "committed examples must pass `surfacer lint`, the gate the docs tell users to run:\n  {}",
        failures.join("\n  ")
    );
}

/// A command path a person cannot type on purpose is not usable, and lint
/// cannot catch it: a numeric suffix or a stray file extension is unique, so
/// it satisfies the schema while reading as machine output.
#[test]
fn example_command_paths_are_typable() {
    let mut offenders = Vec::new();

    for ir in example_irs() {
        let text = std::fs::read_to_string(&ir).expect("example must be readable");
        let doc: serde_json::Value = serde_json::from_str(&text).expect("example must be JSON");
        let name = ir.file_name().unwrap_or_default().to_string_lossy().to_string();

        for op in doc["operations"].as_array().unwrap_or(&Vec::new()) {
            let path: Vec<String> = op["commandPath"]
                .as_array()
                .map(|segs| {
                    segs.iter()
                        .map(|s| s.as_str().unwrap_or_default().to_string())
                        .collect()
                })
                .unwrap_or_default();
            let joined = path.join(" ");

            if path.iter().any(|seg| {
                seg.ends_with("-html") || seg.ends_with("-htm") || seg.ends_with("-php")
            }) {
                offenders.push(format!("{name}: `{joined}` carries a file extension"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "a command path is what a person types, so it should not read like a URL:\n  {}",
        offenders.join("\n  ")
    );
}
