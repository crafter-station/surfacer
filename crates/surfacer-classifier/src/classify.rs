use crate::{ClassificationFeatures, ClassifierBucket, Confidence};
use surfacer_probe::har::parse_har;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationResult {
    pub bucket: ClassifierBucket,
    pub confidence: Confidence,
    pub features: ClassificationFeatures,
}

pub enum TechniqueDecision {
    HttpOnly,
    AxOnly,
    Hybrid,
    Abort,
}

pub fn classify(
    _capture: &surfacer_probe::ProbeCapture,
    har_bytes: &[u8],
    ax_tree: Option<&str>,
) -> anyhow::Result<ClassificationResult> {
    let har = parse_har(har_bytes)?;
    let features = crate::features::extract_features(&har, ax_tree);
    let (bucket, confidence) = if features.hostile_detected {
        (ClassifierBucket::Hostile, Confidence::High)
    } else if features.post_form_body_count >= 10
        && features.post_html_response_count >= 10
        && features.json_responses <= 5
    {
        (ClassifierBucket::FormSessionLegacy, Confidence::High)
    } else if features.cors_preflight_count >= 10
        && features.xhr_fetch >= 50
        && features.json_responses >= 30
    {
        (ClassifierBucket::RestModernSpa, Confidence::High)
    } else if features.graphql_endpoints > 0 && features.json_responses > 0 {
        (ClassifierBucket::GraphqlIntrospectable, Confidence::High)
    } else if features.xhr_fetch == 0
        && features.mutating_requests == 0
        && features.total_requests < 10
    {
        (ClassifierBucket::HtmlRendered, Confidence::High)
    } else if features.xhr_fetch == 0 && features.ax_interactive_nodes >= 5 {
        (ClassifierBucket::AxOnly, Confidence::High)
    } else {
        (ClassifierBucket::Inconclusive, Confidence::Low)
    };

    Ok(ClassificationResult {
        bucket,
        confidence,
        features,
    })
}

pub fn feature_summary(result: &ClassificationResult) -> String {
    let bucket = match result.bucket {
        ClassifierBucket::FormSessionLegacy => "FormSessionLegacy",
        ClassifierBucket::RestModernSpa => "RestModernSpa",
        ClassifierBucket::GraphqlIntrospectable => "GraphqlIntrospectable",
        ClassifierBucket::AxOnly => "AxOnly",
        ClassifierBucket::HtmlRendered => "HTML rendered",
        ClassifierBucket::Hostile => "Hostile",
        ClassifierBucket::Inconclusive => "Inconclusive",
    };
    let confidence = match result.confidence {
        Confidence::High => "high",
        Confidence::Medium => "medium",
        Confidence::Low => "low",
    };
    let f = &result.features;

    format!(
        "{bucket} with {confidence} confidence: {total} requests, {json} JSON responses, {html} HTML responses, {xhr} XHR/Fetch, {form_posts} form POSTs, {post_html} POST->HTML, {preflights} CORS preflights, {hosts} unique hosts, {ax} AX interactive nodes.",
        total = f.total_requests,
        json = f.json_responses,
        html = f.html_responses,
        xhr = f.xhr_fetch,
        form_posts = f.post_form_body_count,
        post_html = f.post_html_response_count,
        preflights = f.cors_preflight_count,
        hosts = f.unique_hosts,
        ax = f.ax_interactive_nodes,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use surfacer_probe::har::HarLog;

    fn sunat_har() -> HarLog {
        parse_har(include_bytes!("../../../fixtures/sunat/capture.har")).unwrap()
    }

    fn sunat_ax() -> &'static str {
        include_str!("../../../fixtures/sunat/ax-final.txt")
    }

    fn synthetic_html_har() -> HarLog {
        parse_har(
            br#"{
              "log": {
                "version": "1.2",
                "entries": [
                  {
                    "_resourceType": "Document",
                    "request": { "method": "GET", "url": "https://example.com/", "headers": [] },
                    "response": {
                      "status": 200,
                      "headers": [{ "name": "content-type", "value": "text/html" }],
                      "content": { "size": 120, "mimeType": "text/html" }
                    }
                  }
                ]
              }
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn test_extract_features_sunat() {
        let har = sunat_har();
        let features = crate::features::extract_features(&har, Some(sunat_ax()));

        assert!(features.json_responses <= 5);
        assert!(features.post_form_body_count >= 10);
        assert!(features.post_html_response_count >= 10);
        assert_eq!(features.cors_preflight_count, 0);
    }

    #[test]
    fn test_classify_sunat() {
        let result = classify(
            &surfacer_probe::ProbeCapture {
                har_path: std::path::PathBuf::new(),
                har_entry_count: 0,
                final_url: None,
                final_title: None,
                ax_pre_path: None,
                ax_final_path: None,
            },
            include_bytes!("../../../fixtures/sunat/capture.har"),
            Some(sunat_ax()),
        )
        .unwrap();

        assert_eq!(result.bucket, ClassifierBucket::FormSessionLegacy);
        assert_eq!(result.confidence, Confidence::High);
    }

    #[test]
    fn test_infer_endpoints_sunat() {
        let endpoints = crate::http_infer::infer_endpoints(&sunat_har());

        assert!(!endpoints.is_empty());
        assert!(endpoints
            .iter()
            .any(|endpoint| endpoint.path.contains("cpelec001Alias")));
    }

    #[test]
    fn test_feature_summary_format() {
        let features =
            crate::features::extract_features(&synthetic_html_har(), Some(r#"button @e1 "Sign in""#));
        let summary = feature_summary(&ClassificationResult {
            bucket: ClassifierBucket::HtmlRendered,
            confidence: Confidence::High,
            features,
        });

        assert!(summary.contains("HTML rendered"));
        assert!(summary.contains("high confidence"));
        assert!(summary.contains("requests"));
        assert!(summary.contains("AX interactive nodes"));
    }
}
