use serde::{Deserialize, Serialize};

use crate::OperationKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpSurface {
    pub endpoints: Vec<HttpEndpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpEndpoint {
    pub namespace: Vec<String>,
    pub method: HttpMethod,
    pub path: String,
    pub description: String,
    pub operation_kind: OperationKind,
    #[serde(default)]
    pub sample_request_content_type: Option<String>,
    #[serde(default)]
    pub sample_response_content_type: Option<String>,
    /// Query parameters observed on this endpoint during recon.
    ///
    /// Defaults to empty so descriptors emitted before parameters existed still
    /// deserialize.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<ParamDescriptor>,
}

/// A query parameter observed during recon.
///
/// Recon sees requests, not documentation, so every field records what was
/// observed rather than what the site guarantees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDescriptor {
    pub name: String,
    /// True when the parameter carried a different value across observations,
    /// which is the strongest available evidence that a caller controls it.
    #[serde(default)]
    pub varies: bool,
    /// A value seen during recon, shown in help as an example.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
    /// How many requests to this endpoint carried the parameter.
    #[serde(default)]
    pub observations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

pub fn derive_operation_kind(method: &HttpMethod) -> OperationKind {
    match method {
        HttpMethod::Get | HttpMethod::Head | HttpMethod::Options => OperationKind::Read,
        HttpMethod::Post | HttpMethod::Put | HttpMethod::Patch | HttpMethod::Delete => {
            OperationKind::Write
        }
    }
}

pub fn normalize_command_path(namespace: &[String]) -> Vec<String> {
    namespace.iter().map(|s| camel_to_kebab(s)).collect()
}

fn camel_to_kebab(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('-');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}
