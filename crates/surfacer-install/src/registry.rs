use anyhow::Context;

pub fn load_registry(home: &std::path::Path) -> anyhow::Result<surfacer_ir::RegistryIndex> {
    surfacer_ir::RegistryIndex::load(&surfacer_ir::registry_path(home))
}

pub fn write_registry(
    home: &std::path::Path,
    index: &surfacer_ir::RegistryIndex,
) -> anyhow::Result<()> {
    index
        .save(&surfacer_ir::registry_path(home))
        .with_context(|| "failed to write local registry")
}

pub fn register_site(
    home: &std::path::Path,
    entry: surfacer_ir::InstalledSiteEntry,
) -> anyhow::Result<()> {
    let mut index = load_registry(home)?;
    index.upsert(entry);
    write_registry(home, &index)
}

pub fn unregister_site(home: &std::path::Path, site_name: &str) -> anyhow::Result<bool> {
    let mut index = load_registry(home)?;
    let removed = index.remove(site_name);
    write_registry(home, &index)?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        register_site(
            home,
            surfacer_ir::InstalledSiteEntry {
                site_name: "sunat".into(),
                ir_path: home.join(".surfacer/sites/sunat/ir.json"),
                shim_path: home.join(".surfacer/bin/sunat"),
            },
        )
        .unwrap();

        let registry = load_registry(home).unwrap();

        assert_eq!(registry.sites.len(), 1);
        assert_eq!(registry.sites[0].site_name, "sunat");
    }
}
