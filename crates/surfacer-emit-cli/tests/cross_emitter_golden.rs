//! Golden test: the Rust path and the TypeScript runtime agree.
//!
//! The README claims "every command gets the same flag handling, the same JSON
//! output, because one emitter wrote all of them". That claim was false when
//! this test was written: the same IR, run through `surfacer exec`, returned
//! thirty structured items, and run through the emitted ts-cli returned raw
//! HTML, because the extractor existed only in Rust.
//!
//! # What "the same" means here, precisely
//!
//! The two paths do not read the same bytes, and pretending otherwise would
//! make this test a fiction. `surfacer exec` shells out to `defuddle` and hands
//! the extractor defuddle's cleaned content. A standalone binary has no
//! defuddle (it is an npm program, and depending on it would void the
//! standalone promise), so the TypeScript runtime fetches the raw page and
//! normalizes it itself.
//!
//! So the claim under test is the useful one: given the same page, the two
//! paths produce the same items. Two fixtures are committed, captured from the
//! same fetch of the same URL:
//!
//!   news-ycombinator-com.html            what a binary fetches (raw)
//!   news-ycombinator-com.defuddled.html  what `surfacer exec` extracts from
//!
//! The Rust side reads the defuddled one, the TypeScript side reads the raw
//! one, and the items must match. That is exactly the property a caller cares
//! about, and it is the property that was false before the runtime existed.
//!
//! Fixtures are committed rather than fetched: a golden test that depends on
//! today's front page fails for reasons that have nothing to do with the code.

use std::process::Command;

/// The committed HN descriptor's `news` extractor, as JSON.
///
/// Read from the example IR rather than hand-built, so a schema change breaks
/// this test instead of letting the two paths drift behind a stale copy.
fn news_extractor() -> surfacer_ir::Extractor {
    let ir_path = repo_root().join("examples/news-ycombinator-com.surfacer.json");
    let descriptor = surfacer_ir::read_ir(&ir_path).expect("example IR should parse");

    descriptor
        .operations
        .iter()
        .find(|op| op.command_path == ["news"])
        .and_then(|op| op.extractor.clone())
        .expect("the news operation should carry an extractor")
}

fn repo_root() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is crates/surfacer-emit-cli.
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

/// The raw page, which is what a standalone binary fetches.
fn raw_fixture() -> std::path::PathBuf {
    repo_root().join("fixtures/extract/news-ycombinator-com.html")
}

/// Defuddle's cleaned content, which is what `surfacer exec` extracts from.
fn defuddled_fixture_html() -> String {
    let path = repo_root().join("fixtures/extract/news-ycombinator-com.defuddled.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// Run the TypeScript runtime over the fixture and return its JSON.
///
/// Executed with `bun`, which is the runtime the repo already uses. When bun is
/// absent the test skips loudly rather than passing quietly: a golden test that
/// silently stops comparing is worse than no golden test, because the pitch it
/// guards keeps looking verified.
fn typescript_items(extractor: &surfacer_ir::Extractor) -> Option<serde_json::Value> {
    if Command::new("bun").arg("--version").output().is_err() {
        eprintln!("SKIPPED: bun is not installed, so the TypeScript side cannot run");
        return None;
    }

    let runtime = surfacer_emit_cli::runtime_source();
    let literal = surfacer_emit_cli::render_extractor(extractor);
    let fixture = raw_fixture();

    // The harness mirrors what the emitted CLI does: inline the runtime, build
    // the extractor literal, extract, and print with the same formatter.
    let program = format!(
        r#"import {{ readFileSync }} from "node:fs";

{runtime}

const EXTRACTOR: Extractor = {literal};

const html = readFileSync({fixture:?}, "utf-8");
const outcome = extractItems(html, EXTRACTOR);
const items = outcome.items;
if (items === undefined) {{
  const failure = outcome.failure;
  console.error("no items: " + (failure !== undefined ? failure.reason : "unknown"));
  process.exit(1);
}}
console.log(formatExtractedJson(items, "https://news.ycombinator.com/news", "Hacker News"));
"#,
        fixture = fixture.to_string_lossy(),
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("golden.ts");
    std::fs::write(&script, program).expect("write harness");

    let output = Command::new("bun")
        .arg("run")
        .arg(&script)
        .output()
        .expect("bun should run");

    assert!(
        output.status.success(),
        "TypeScript runtime failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    Some(serde_json::from_str(&stdout).expect("TypeScript output should be JSON"))
}

/// Run the Rust extractor over the same fixture and shape it the same way.
///
/// `surfacer_emit_cli` cannot depend on `surfacer-app`, where `extract_items`
/// lives, so the traversal is reproduced here against the same `scraper`
/// version. That is a real limitation of this test: it proves the two
/// implementations of the algorithm agree, not that this crate calls the app's
/// copy. The app's copy stays under its own tests.
fn rust_items(extractor: &surfacer_ir::Extractor, html: &str) -> serde_json::Value {
    use scraper::{Html, Selector};
    use std::collections::BTreeMap;
    use surfacer_ir::{ExtractedItem, ExtractedValue, FieldSource, FieldType};

    let surfacer_ir::Extractor::List(list) = extractor else {
        panic!("the news extractor should be a list");
    };

    let css = list
        .item_pattern
        .css_selector
        .as_deref()
        .expect("the news pattern should carry a CSS selector");

    let doc = Html::parse_document(html);
    let item_sel = Selector::parse(css).expect("selector should parse");

    let items: Vec<ExtractedItem> = doc
        .select(&item_sel)
        .enumerate()
        .map(|(i, element)| {
            let mut fields = BTreeMap::new();
            for field_def in &list.fields {
                let value = match &field_def.source {
                    FieldSource::Css(source) => {
                        let Ok(sel) = Selector::parse(&source.selector) else {
                            fields.insert(field_def.name.clone(), ExtractedValue::Missing);
                            continue;
                        };
                        match element.select(&sel).next() {
                            None => ExtractedValue::Missing,
                            Some(target) => {
                                let raw = match &source.attribute {
                                    Some(attr) => target.value().attr(attr).map(|s| s.to_string()),
                                    None => Some(
                                        target.text().collect::<Vec<_>>().join("").trim().to_string(),
                                    ),
                                };
                                match raw {
                                    None => ExtractedValue::Missing,
                                    Some(raw) if raw.is_empty() => ExtractedValue::Missing,
                                    Some(raw) => match field_def.field_type {
                                        FieldType::Text | FieldType::DateTime => {
                                            ExtractedValue::Text(raw)
                                        }
                                        FieldType::Url => ExtractedValue::Url(raw),
                                        FieldType::Number => {
                                            let digits: String = raw
                                                .chars()
                                                .filter(|c| c.is_ascii_digit() || *c == '.')
                                                .collect();
                                            digits
                                                .parse::<f64>()
                                                .map(ExtractedValue::Number)
                                                .unwrap_or(ExtractedValue::Text(raw))
                                        }
                                    },
                                }
                            }
                        }
                    }
                    // extract.rs returns Missing for every AX source.
                    FieldSource::AxTree(_) => ExtractedValue::Missing,
                };
                fields.insert(field_def.name.clone(), value);
            }
            ExtractedItem {
                index: i + 1,
                fields,
            }
        })
        .filter(|item| {
            item.fields
                .values()
                .any(|v| !matches!(v, ExtractedValue::Missing))
        })
        .collect();

    serde_json::json!({
        "url": "https://news.ycombinator.com/news",
        "title": "Hacker News",
        "itemCount": items.len(),
        "items": items,
    })
}

#[test]
fn rust_and_typescript_produce_the_same_items() {
    let extractor = news_extractor();

    // TypeScript reads the raw page and normalizes it; Rust reads what defuddle
    // already normalized. Same page, same items, different route to the input.
    let Some(ts) = typescript_items(&extractor) else {
        return;
    };
    let rust = rust_items(&extractor, &defuddled_fixture_html());

    // The envelope: same keys, same values. `itemCount` is part of the
    // comparison rather than a separate assertion so a count that agrees while
    // the items differ cannot pass.
    assert_eq!(
        rust["url"], ts["url"],
        "the two paths disagree on the url they report"
    );
    assert_eq!(
        rust["title"], ts["title"],
        "the two paths disagree on the title they report"
    );
    assert_eq!(
        rust["itemCount"], ts["itemCount"],
        "the two paths extracted a different number of items:\nrust: {}\nts:   {}",
        rust["itemCount"], ts["itemCount"],
    );

    assert_eq!(
        rust["items"], ts["items"],
        "the two paths produced different items.\nrust: {}\nts:   {}",
        serde_json::to_string_pretty(&rust["items"]).unwrap(),
        serde_json::to_string_pretty(&ts["items"]).unwrap(),
    );
}

/// The fields that differ between the paths, named rather than ignored.
///
/// `wordCount` is the one. On the Rust path it comes from defuddle, which
/// computes it while parsing the page it cleaned. The TypeScript runtime has no
/// defuddle, so it counts words in the text it produced itself. The two numbers
/// describe different inputs and are not expected to match; asserting equality
/// would either fail forever or force one side to fake the other's number.
///
/// The rest of `ExecResult` does have to match, which is what this asserts.
#[test]
fn exec_result_shape_matches_except_word_count() {
    if Command::new("bun").arg("--version").output().is_err() {
        eprintln!("SKIPPED: bun is not installed");
        return;
    }

    let runtime = surfacer_emit_cli::runtime_source();
    let program = format!(
        r#"{runtime}

const result = execResultFromHtml(200, "https://example.com/x", "Example", "<p>alpha beta</p><p>gamma</p>");
console.log(JSON.stringify(result));
"#
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("exec.ts");
    std::fs::write(&script, program).expect("write harness");

    let output = Command::new("bun")
        .arg("run")
        .arg(&script)
        .output()
        .expect("bun should run");
    assert!(
        output.status.success(),
        "runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("output should be JSON");

    // The keys ExecResult serializes, in the camelCase serde emits.
    for key in ["status", "url", "title", "content", "wordCount"] {
        assert!(
            value.get(key).is_some(),
            "the TypeScript ExecResult is missing `{key}`, which the Rust one has"
        );
    }

    assert_eq!(value["status"], 200);
    assert_eq!(value["url"], "https://example.com/x");
    assert_eq!(value["title"], "Example");
    // html_to_text collapses the tags and joins the surviving lines, which both
    // implementations do identically.
    assert_eq!(value["content"], "alpha beta\ngamma");

    // wordCount is computed, not inherited from defuddle. Asserting the count
    // it actually produces keeps the difference visible rather than silent.
    assert_eq!(
        value["wordCount"], 3,
        "the TypeScript runtime counts words in its own text; the Rust path takes defuddle's number"
    );
}

/// The parameter rule is one rule, expressed twice, and the two agree.
///
/// The CLI path exits 64 with a JSON body; the MCP path cannot exit, so it
/// returns the same body as a tool error. This asserts the bodies match, which
/// is the property that actually matters to a caller parsing them.
#[test]
fn missing_parameter_error_is_identical_across_targets() {
    if Command::new("bun").arg("--version").output().is_err() {
        eprintln!("SKIPPED: bun is not installed");
        return;
    }

    let runtime = surfacer_emit_cli::runtime_source();
    let program = format!(
        r#"{runtime}

const required: Param[] = [{{ name: "id", example: "Hunter17", observations: 1, varies: false }}];
const missing = missingParams(required, new Set<string>());
console.log(JSON.stringify(missingParamsError("user", missing)));
"#
    );

    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("params.ts");
    std::fs::write(&script, program).expect("write harness");

    let output = Command::new("bun")
        .arg("run")
        .arg(&script)
        .output()
        .expect("bun should run");
    assert!(
        output.status.success(),
        "runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let from_runtime: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("runtime output should be JSON");

    // The MCP emitter writes its own copy of this rule, because the emitted
    // server is JavaScript and cannot import the TypeScript runtime. Emitting
    // the same descriptor and running that copy is what proves they agree.
    let ir_path = repo_root().join("examples/news-ycombinator-com.surfacer.json");
    let descriptor = surfacer_ir::read_ir(&ir_path).expect("example IR should parse");
    let server = surfacer_emit_cli::emit_mcp_server(&descriptor);

    let start = server
        .find("function missingParams(")
        .expect("the emitted server should carry the check");
    let end = server
        .find("async function call(")
        .expect("the emitted server should carry call()");
    let functions = &server[start..end];

    let node_program = format!(
        r#"{functions}
const required = [{{ name: "id", example: "Hunter17", observations: 1, varies: false }}];
console.log(JSON.stringify(missingParamsError("user", missingParams(required, {{}}))));
"#
    );
    let node_script = dir.path().join("mcp-check.mjs");
    std::fs::write(&node_script, node_program).expect("write node harness");

    let node_output = Command::new("node")
        .arg(&node_script)
        .output()
        .expect("node should run");
    assert!(
        node_output.status.success(),
        "emitted MCP check failed: {}",
        String::from_utf8_lossy(&node_output.stderr)
    );

    let from_mcp: serde_json::Value =
        serde_json::from_slice(&node_output.stdout).expect("MCP output should be JSON");

    assert_eq!(
        from_runtime, from_mcp,
        "the CLI runtime and the emitted MCP server disagree on the missing-parameter body"
    );
}
