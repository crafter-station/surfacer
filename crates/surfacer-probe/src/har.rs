use anyhow::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarLog {
    #[serde(default)]
    pub log: HarLogInner,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarLogInner {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub entries: Vec<HarEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarEntry {
    #[serde(rename = "_resourceType", default)]
    pub resource_type: Option<String>,
    #[serde(default)]
    pub request: HarRequest,
    #[serde(default)]
    pub response: HarResponse,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub headers: Vec<HarHeader>,
    #[serde(default)]
    pub post_data: Option<HarPostData>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarResponse {
    #[serde(default)]
    pub status: u16,
    #[serde(default)]
    pub headers: Vec<HarHeader>,
    #[serde(default)]
    pub content: HarContent,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HarHeader {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarContent {
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarPostData {
    #[serde(default)]
    pub mime_type: String,
    #[serde(default)]
    pub text: Option<String>,
}

pub fn parse_har(bytes: &[u8]) -> anyhow::Result<HarLog> {
    serde_json::from_slice(bytes).context("failed to parse HAR JSON")
}

#[cfg(test)]
mod tests {
    use super::parse_har;
    use std::path::PathBuf;

    #[test]
    fn parses_sunat_fixture() {
        let fixture_path = workspace_root().join("fixtures/sunat/capture.har");
        let fixture = std::fs::read(&fixture_path).expect("read fixture");
        let har = parse_har(&fixture).expect("parse fixture");

        assert_eq!(har.log.version, "1.2");
        assert_eq!(har.log.entries.len(), 412);
        assert_eq!(har.log.entries[0].resource_type.as_deref(), Some("Document"));
        assert_eq!(har.log.entries[0].request.method, "GET");
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }
}
