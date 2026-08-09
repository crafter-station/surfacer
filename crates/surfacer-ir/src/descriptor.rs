use serde::{Deserialize, Serialize};

use crate::{AxSurface, Extractor, HttpSurface, Provenance, SiteMeta};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SiteDescriptor {
    pub meta: SiteMeta,
    pub provenance: Provenance,
    pub operations: Vec<OperationDescriptor>,
    #[serde(default)]
    pub http: Option<HttpSurface>,
    #[serde(default)]
    pub ax: Option<AxSurface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationDescriptor {
    pub command_path: Vec<String>,
    pub summary: String,
    pub description: String,
    pub operation_kind: OperationKind,
    pub transport: OperationTransport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor: Option<Extractor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OperationTransport {
    Http(HttpOperation),
    Ax(AxOperation),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpOperation {
    pub endpoint_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AxOperation {
    pub action_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    Read,
    Write,
    Other,
}
