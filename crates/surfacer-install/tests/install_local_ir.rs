use surfacer_install::{install::install_site, registry, resolve::resolve_ir, resolve::ResolvedIrSource};

fn descriptor_json(site: &str) -> serde_json::Value {
    serde_json::json!({
        "meta": {
            "siteName": site,
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
        "operations": [{
            "commandPath": ["list"],
            "summary": "list",
            "description": "list",
            "operationKind": "read",
            "transport": { "kind": "http", "endpointIndex": 0 }
        }],
        "http": {
            "endpoints": [{
                "namespace": ["list"],
                "method": "GET",
                "path": "/list",
                "description": "list",
                "operationKind": "read"
            }]
        }
    })
}

/// Write a descriptor to disk and return its path.
fn write_ir(dir: &std::path::Path, site: &str) -> std::path::PathBuf {
    let path = dir.join(format!("{site}.surfacer.json"));
    std::fs::write(&path, descriptor_json(site).to_string()).expect("write IR");
    path
}

#[test]
fn installs_an_ir_from_a_local_path() {
    let work = tempfile::tempdir().expect("tempdir");
    let home = tempfile::tempdir().expect("home");
    let ir_path = write_ir(work.path(), "example-com");

    let resolved = resolve_ir(ir_path.to_str().expect("utf-8 path")).expect("resolve");
    assert_eq!(
        resolved,
        ResolvedIrSource::LocalPath(ir_path.clone()),
        "an existing path must resolve as local, not as a registry name"
    );

    let descriptor: surfacer_ir::SiteDescriptor =
        serde_json::from_value(descriptor_json("example-com")).expect("descriptor");
    let installed = install_site(&descriptor, &ir_path, home.path()).expect("install");

    assert_eq!(installed.site_name, "example-com");
    assert_eq!(installed.command_count, 1);
    assert!(
        installed.ir_path.exists(),
        "the descriptor must be copied into the site directory, not merely referenced"
    );

    // The copy must be readable as a descriptor, not just present as bytes.
    let reread = surfacer_ir::read_ir(&installed.ir_path).expect("re-read installed IR");
    assert_eq!(reread.meta.site_name, "example-com");
}

#[test]
fn installing_twice_overwrites_rather_than_failing() {
    let work = tempfile::tempdir().expect("tempdir");
    let home = tempfile::tempdir().expect("home");
    let ir_path = write_ir(work.path(), "example-com");
    let descriptor: surfacer_ir::SiteDescriptor =
        serde_json::from_value(descriptor_json("example-com")).expect("descriptor");

    install_site(&descriptor, &ir_path, home.path()).expect("first install");
    let second = install_site(&descriptor, &ir_path, home.path())
        .expect("reinstalling an already-installed site must succeed");

    assert_eq!(second.site_name, "example-com");
}

#[test]
fn the_registry_round_trips_an_installed_site() {
    let home = tempfile::tempdir().expect("home");

    let empty = registry::load_registry(home.path()).expect("load empty registry");
    assert!(
        empty.sites.is_empty(),
        "a fresh home must start with no registered sites"
    );

    registry::register_site(
        home.path(),
        surfacer_ir::InstalledSiteEntry {
            site_name: "example-com".into(),
            ir_path: home.path().join("sites/example-com/ir.json"),
            shim_path: home.path().join("bin/example-com"),
        },
    )
    .expect("register");

    let loaded = registry::load_registry(home.path()).expect("load registry");
    assert_eq!(loaded.sites.len(), 1);
    assert_eq!(loaded.sites[0].site_name, "example-com");

    let removed = registry::unregister_site(home.path(), "example-com").expect("unregister");
    assert!(removed, "unregistering a present site must report success");

    let after = registry::load_registry(home.path()).expect("reload");
    assert!(after.sites.is_empty(), "the site must be gone after removal");

    let missing = registry::unregister_site(home.path(), "never-installed").expect("unregister");
    assert!(
        !missing,
        "unregistering an absent site must report false rather than erroring"
    );
}
