use webctl_probe::har::HarLog;

fn hn_har() -> Option<HarLog> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../webctl-recon-news-ycombinator-com/capture.har"
    );
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

#[test]
fn derives_params_from_the_hacker_news_capture() {
    let Some(har) = hn_har() else {
        eprintln!("skipping: recon capture not present");
        return;
    };
    let endpoints = webctl_classifier::http_infer::infer_endpoints(&har);
    let with_params: Vec<_> = endpoints.iter().filter(|e| !e.params.is_empty()).collect();

    println!("endpoints: {}", endpoints.len());
    for e in &with_params {
        println!("  {:?} {}", e.method, e.path);
        for p in &e.params {
            println!(
                "      {:<14} varies={:<5} obs={:<3} example={:?}",
                p.name, p.varies, p.observations, p.example
            );
        }
    }

    let names: Vec<&str> = with_params
        .iter()
        .flat_map(|e| e.params.iter())
        .map(|p| p.name.as_str())
        .collect();

    assert!(
        names.contains(&"id"),
        "the /user endpoint carries an `id` parameter in the capture; got {names:?}"
    );
    assert!(
        !names.iter().any(|n| n.len() >= 16 && n.chars().all(|c| c.is_ascii_alphanumeric())),
        "opaque cache-buster tokens must not survive as parameters; got {names:?}"
    );
}

#[test]
fn help_and_emitted_config_surface_the_params() {
    let Some(har) = hn_har() else {
        eprintln!("skipping: recon capture not present");
        return;
    };
    let endpoints = webctl_classifier::http_infer::infer_endpoints(&har);

    // Rebuild the shipped descriptor with freshly-derived endpoints so the
    // fixture exercises the same path a new recon would produce.
    let shipped = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/news-ycombinator-com.webctl.json");
    let raw = std::fs::read_to_string(shipped).expect("shipped IR");
    let mut descriptor: webctl_ir::SiteDescriptor = serde_json::from_str(&raw).expect("parse IR");
    descriptor.http = Some(webctl_ir::HttpSurface { endpoints });

    let help = webctl_emit_cli::build_help_text(&descriptor);
    println!("--- HELP ---\n{help}");
    assert!(help.contains("id="), "help must list observed params as flags");

    let config = webctl_emit_cli::emit_executor_config(&descriptor);
    assert!(
        config.contains("// Parameters observed during recon:"),
        "emitted config must document observed params"
    );
    assert!(
        !config.contains("?id={id}"),
        "the recon-time query template must not leak into the base URL"
    );
}
