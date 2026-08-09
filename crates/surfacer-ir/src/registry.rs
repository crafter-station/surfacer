use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallRecord {
    pub site_name: String,
    pub ir_path: std::path::PathBuf,
    pub shim_path: std::path::PathBuf,
    pub installed_at: String,
    pub source: InstallSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum InstallSource {
    LocalPath(LocalPathSource),
    GithubRepo(GithubRepoSource),
    RegistryName(RegistryNameSource),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPathSource {
    pub path: std::path::PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubRepoSource {
    pub repo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryNameSource {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryIndex {
    pub sites: Vec<InstalledSiteEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSiteEntry {
    pub site_name: String,
    pub ir_path: std::path::PathBuf,
    pub shim_path: std::path::PathBuf,
}

impl RegistryIndex {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self { sites: Vec::new() });
        }
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn find(&self, site_name: &str) -> Option<&InstalledSiteEntry> {
        self.sites.iter().find(|s| s.site_name == site_name)
    }

    pub fn upsert(&mut self, entry: InstalledSiteEntry) {
        if let Some(existing) = self
            .sites
            .iter_mut()
            .find(|s| s.site_name == entry.site_name)
        {
            *existing = entry;
        } else {
            self.sites.push(entry);
        }
    }

    pub fn remove(&mut self, site_name: &str) -> bool {
        let before = self.sites.len();
        self.sites.retain(|s| s.site_name != site_name);
        self.sites.len() < before
    }
}
