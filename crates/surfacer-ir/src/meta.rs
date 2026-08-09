use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteMeta {
    pub site_name: String,
    pub display_name: String,
    pub source_url: String,
    pub ir_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub generated_at: String,
    pub technique: ProvenanceTechnique,
    pub classifier_bucket: String,
    pub probe_duration_sec: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProvenanceTechnique {
    Http,
    Ax,
    Hybrid,
}
