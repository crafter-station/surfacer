#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedIrSource {
    LocalPath(std::path::PathBuf),
    GithubRepo(String),
    RegistryName(String),
}

pub fn resolve_ir(name_or_path: &str) -> anyhow::Result<ResolvedIrSource> {
    let path = std::path::PathBuf::from(name_or_path);
    if path.exists() {
        return Ok(ResolvedIrSource::LocalPath(path));
    }
    if name_or_path.starts_with("crafter-station/") || name_or_path.contains('/') {
        return Ok(ResolvedIrSource::GithubRepo(name_or_path.to_string()));
    }
    Ok(ResolvedIrSource::RegistryName(name_or_path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_local_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site.json");
        std::fs::write(&path, "{}").unwrap();

        let resolved = resolve_ir(path.to_str().unwrap()).unwrap();

        assert_eq!(resolved, ResolvedIrSource::LocalPath(path));
    }

    #[test]
    fn test_resolve_github_repo() {
        let resolved = resolve_ir("crafter-station/surfacer-ir-sunat").unwrap();

        assert_eq!(
            resolved,
            ResolvedIrSource::GithubRepo("crafter-station/surfacer-ir-sunat".to_string())
        );
    }

    #[test]
    fn test_resolve_registry_name() {
        let resolved = resolve_ir("sunat").unwrap();

        assert_eq!(resolved, ResolvedIrSource::RegistryName("sunat".to_string()));
    }
}
