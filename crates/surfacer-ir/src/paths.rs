use crate::SiteDescriptor;
use std::path::{Path, PathBuf};

const SURFACER_DIR: &str = ".surfacer";
const SITES_DIR: &str = "sites";
const REGISTRY_FILE: &str = "registry.json";

pub fn surfacer_home(home: &Path) -> PathBuf {
    home.join(SURFACER_DIR)
}

pub fn write_ir(out_path: impl AsRef<Path>, descriptor: &SiteDescriptor) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(descriptor)?;
    if let Some(parent) = out_path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, json)?;
    Ok(())
}

pub fn read_ir(path: impl AsRef<Path>) -> anyhow::Result<SiteDescriptor> {
    let content = std::fs::read_to_string(path)?;
    let descriptor: SiteDescriptor = serde_json::from_str(&content)?;
    Ok(descriptor)
}

pub fn registry_path(home: &Path) -> PathBuf {
    surfacer_home(home).join(REGISTRY_FILE)
}

pub fn site_dir(home: &Path, site_name: &str) -> PathBuf {
    surfacer_home(home).join(SITES_DIR).join(site_name)
}

pub fn site_ir_path(home: &Path, site_name: &str) -> PathBuf {
    site_dir(home, site_name).join("ir.json")
}

pub fn site_meta_path(home: &Path, site_name: &str) -> PathBuf {
    site_dir(home, site_name).join("meta.json")
}
