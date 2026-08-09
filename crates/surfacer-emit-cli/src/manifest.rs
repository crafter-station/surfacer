use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShimManifest {
    pub site_name: String,
    pub surfacer_path: std::path::PathBuf,
    pub shim_path: std::path::PathBuf,
    pub compiled_at: String,
}

pub fn write_manifest(path: impl AsRef<Path>, manifest: &ShimManifest) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(manifest)?;
    std::fs::write(path.as_ref(), json)
        .with_context(|| format!("failed to write manifest to {}", path.as_ref().display()))?;
    Ok(())
}

pub fn read_manifest(path: impl AsRef<Path>) -> anyhow::Result<ShimManifest> {
    let content = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("failed to read manifest from {}", path.as_ref().display()))?;
    let manifest = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse manifest from {}", path.as_ref().display()))?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrips_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shim-manifest.json");
        let manifest = ShimManifest {
            site_name: "sunat".into(),
            surfacer_path: "surfacer".into(),
            shim_path: dir.path().join("sunat"),
            compiled_at: "1712966400".into(),
        };

        write_manifest(&path, &manifest).unwrap();
        let loaded = read_manifest(&path).unwrap();

        assert_eq!(loaded, manifest);
    }
}
