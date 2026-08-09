use std::collections::BTreeSet;

use surfacer_probe::har::{HarEntry, HarHeader, HarLog};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassificationFeatures {
    pub total_requests: usize,
    pub json_responses: usize,
    pub html_responses: usize,
    pub xhr_fetch: usize,
    pub graphql_endpoints: usize,
    pub post_form_body_count: usize,
    pub post_json_body_count: usize,
    pub post_html_response_count: usize,
    pub cors_preflight_count: usize,
    pub mutating_requests: usize,
    pub bearer_auth_count: usize,
    pub unique_hosts: usize,
    pub ax_interactive_nodes: usize,
    pub ax_size_bytes: usize,
    pub hostile_detected: bool,
}

pub fn extract_features(har: &HarLog, ax_text: Option<&str>) -> ClassificationFeatures {
    let mut features = ClassificationFeatures {
        total_requests: har.log.entries.len(),
        ax_size_bytes: ax_text.map(str::len).unwrap_or_default(),
        ..ClassificationFeatures::default()
    };
    let mut hosts = BTreeSet::new();

    for entry in &har.log.entries {
        let method = entry.request.method.to_ascii_uppercase();
        let request_content_type = header_value(&entry.request.headers, "content-type")
            .or_else(|| entry.request.post_data.as_ref().map(|post| post.mime_type.as_str()))
            .unwrap_or("")
            .to_ascii_lowercase();
        let response_content_type = response_content_type(entry);
        let url_lower = entry.request.url.to_ascii_lowercase();

        if is_json_content_type(&response_content_type) {
            features.json_responses += 1;
        }
        if response_content_type.starts_with("text/html") {
            features.html_responses += 1;
        }
        if entry
            .resource_type
            .as_deref()
            .is_some_and(|value| matches!(value, "XHR" | "Fetch"))
        {
            features.xhr_fetch += 1;
        }
        if url_lower.contains("graphql") || url_lower.contains("gql") {
            features.graphql_endpoints += 1;
        }
        if method == "POST" && is_form_content_type(&request_content_type) {
            features.post_form_body_count += 1;
        }
        if method == "POST" && is_json_content_type(&request_content_type) {
            features.post_json_body_count += 1;
        }
        if method == "POST" && response_content_type.starts_with("text/html") {
            features.post_html_response_count += 1;
        }
        if method == "OPTIONS" {
            features.cors_preflight_count += 1;
        }
        if matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE") {
            features.mutating_requests += 1;
        }
        if header_value(&entry.request.headers, "authorization")
            .is_some_and(|value| value.trim_start().to_ascii_lowercase().starts_with("bearer "))
        {
            features.bearer_auth_count += 1;
        }
        if let Ok(url) = url::Url::parse(&entry.request.url) {
            if let Some(host) = url.host_str() {
                hosts.insert(host.to_string());
            }
        }
        if hostile_entry_detected(entry) {
            features.hostile_detected = true;
        }
    }

    features.unique_hosts = hosts.len();
    features.ax_interactive_nodes = ax_text.map(count_ax_interactive_nodes).unwrap_or_default();
    features
}

fn header_value<'a>(headers: &'a [HarHeader], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(name))
        .map(|header| header.value.as_str())
}

fn response_content_type(entry: &HarEntry) -> String {
    header_value(&entry.response.headers, "content-type")
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            entry.response
                .content
                .mime_type
                .as_ref()
                .map(|value| value.to_ascii_lowercase())
        })
        .unwrap_or_default()
}

fn is_json_content_type(content_type: &str) -> bool {
    let value = content_type.to_ascii_lowercase();
    value.starts_with("application/json") || value.contains("+json")
}

fn is_form_content_type(content_type: &str) -> bool {
    content_type
        .to_ascii_lowercase()
        .starts_with("application/x-www-form-urlencoded")
}

fn hostile_entry_detected(entry: &HarEntry) -> bool {
    let status = entry.response.status;
    let headers = &entry.response.headers;

    if header_value(headers, "cf-mitigated")
        .is_some_and(|value| value.to_ascii_lowercase().contains("challenge"))
    {
        return true;
    }

    if status >= 400
        && headers.iter().any(|header| {
            header.name.eq_ignore_ascii_case("set-cookie")
                && {
                    let value = header.value.to_ascii_lowercase();
                    value.contains("cf_clearance=")
                        || value.contains("__cf_bm=")
                        || value.contains("datadome=")
                }
        })
    {
        return true;
    }

    let haystack = headers
        .iter()
        .flat_map(|header| [header.name.as_str(), header.value.as_str()])
        .chain(std::iter::once(entry.request.url.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    (status == 403 || status == 429 || status == 503)
        && (haystack.contains("captcha")
            || haystack.contains("challenge")
            || haystack.contains("arkose")
            || haystack.contains("datadome"))
}

fn count_ax_interactive_nodes(ax_text: &str) -> usize {
    ax_text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            ["link @e", "button @e", "textbox @e", "menuitem @e"]
                .iter()
                .any(|prefix| trimmed.starts_with(prefix))
        })
        .count()
}
