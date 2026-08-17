//! The site's copy of the examples must match the repository's.
//!
//! `www/` needs the descriptors inside its own build root, so `bun run sync`
//! copies them there. A copy with no check is a copy that drifts, and this one
//! did: two examples were fixed in `examples/` and shipped, while the site kept
//! serving the broken versions, including seven colliding command paths and an
//! endpoint that returns 404.
//!
//! `lib/ir.ts` already claimed "a CI check fails if the two ever disagree".
//! This is that check. It did not exist when the comment was written.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_site_serves_the_same_descriptors_the_cli_ships() {
    let source = repo_root().join("examples");
    let site = repo_root().join("www/examples");

    if !site.is_dir() {
        // The site is optional in a checkout that only builds the CLI.
        return;
    }

    let mut mismatches = Vec::new();
    let mut checked = 0;

    for entry in std::fs::read_dir(&source).expect("examples directory must exist") {
        let path = entry.expect("readable entry").path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if !name.ends_with(".surfacer.json") {
            continue;
        }

        let mirrored = site.join(&name);
        if !mirrored.exists() {
            mismatches.push(format!("{name}: missing from www/examples"));
            continue;
        }

        // Compare parsed JSON rather than bytes, so formatting alone is not a
        // failure while any change in content is.
        let a: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("readable"))
                .expect("source example must be valid JSON");
        let b: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&mirrored).expect("readable"))
                .expect("mirrored example must be valid JSON");

        if a != b {
            mismatches.push(format!("{name}: differs from examples/"));
        }
        checked += 1;
    }

    assert!(checked > 0, "expected at least one example to compare");
    assert!(
        mismatches.is_empty(),
        "www/examples is stale, so the site would serve descriptors the CLI does not ship.\n  {}\n\nRun `bun run sync` in www/.",
        mismatches.join("\n  ")
    );
}
