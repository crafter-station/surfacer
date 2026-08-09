use std::collections::BTreeMap;

use anyhow::Context;
use regex::Regex;
use scraper::{Html, Selector};
use tokio::process::Command;
use surfacer_ir::{
    CssFieldSource, Extractor, FieldDef, FieldSource, FieldType, ItemPattern, ListExtractor,
    PaginationDef, PaginationStrategy, PatternStrategy,
};

const MIN_REPEATING_ITEMS: usize = 3;
const MAX_SAMPLE_ITEMS: usize = 3;

#[derive(Debug)]
struct DetectedPattern {
    selector: String,
    sample_html: Vec<String>,
    item_count: usize,
}

pub async fn detect_extractor(html: &str, url: &str) -> Option<Extractor> {
    let patterns = detect_repeating_patterns(html);

    let best = patterns.into_iter().max_by_key(|p| p.item_count)?;

    if best.item_count < MIN_REPEATING_ITEMS {
        return None;
    }

    let fields = match name_fields_with_llm(&best.sample_html, url).await {
        Ok(f) if !f.is_empty() => {
            tracing::debug!("LLM named {} fields for {url}", f.len());
            f
        }
        Ok(_) => {
            tracing::debug!("LLM returned empty fields for {url}, using heuristic");
            infer_fields_heuristic(&best.sample_html)
        }
        Err(e) => {
            tracing::debug!("LLM field naming failed for {url}: {e}, using heuristic");
            infer_fields_heuristic(&best.sample_html)
        }
    };

    if fields.is_empty() {
        return None;
    }

    let pagination = detect_pagination(html);

    Some(Extractor::List(ListExtractor {
        item_pattern: ItemPattern {
            strategy: PatternStrategy::Css,
            css_selector: Some(best.selector),
            ax_role: None,
            ax_name_pattern: None,
        },
        fields,
        pagination,
    }))
}

fn detect_repeating_patterns(html: &str) -> Vec<DetectedPattern> {
    let doc = Html::parse_document(html);
    let mut patterns = Vec::new();

    let candidates = [
        ("tr:has(> td > span > a[href^=\"https://\"])", "table row with external link"),
        ("tr:has(> td > a[href^=\"https://\"])", "table row with link"),
        ("li:has(> a)", "list item with link"),
        ("div:has(> a)", "div with link"),
        ("article", "article element"),
    ];

    for (selector_str, _desc) in &candidates {
        let Ok(sel) = Selector::parse(selector_str) else {
            continue;
        };

        let elements: Vec<_> = doc.select(&sel).collect();
        if elements.len() < MIN_REPEATING_ITEMS {
            continue;
        }

        let samples: Vec<String> = elements
            .iter()
            .take(MAX_SAMPLE_ITEMS)
            .map(|el| el.html())
            .collect();

        patterns.push(DetectedPattern {
            selector: selector_str.to_string(),
            sample_html: samples,
            item_count: elements.len(),
        });
    }

    patterns
}

async fn name_fields_with_llm(samples: &[String], url: &str) -> anyhow::Result<Vec<FieldDef>> {
    let samples_text = samples
        .iter()
        .enumerate()
        .map(|(i, s)| format!("Item {}:\n{}", i + 1, truncate(s, 500)))
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        r#"You are analyzing HTML items extracted from {url}.

Here are {count} sample items from a repeating list on the page:

{samples_text}

For each distinct piece of information in these items, output a JSON array of field definitions. Each field should have:
- "name": a short camelCase name (title, url, author, points, domain, date, price, description, etc.)
- "type": one of "text", "url", "number", "dateTime"
- "cssSelector": a CSS selector relative to the item that extracts this field
- "attribute": null for text content, or "href" for URLs

Output ONLY the JSON array, nothing else. Example:
[
  {{"name": "title", "type": "text", "cssSelector": "a", "attribute": null}},
  {{"name": "url", "type": "url", "cssSelector": "a", "attribute": "href"}}
]

JSON:"#,
        count = samples.len()
    );

    let output = Command::new("claude")
        .args(["-p", "--model", "haiku", &prompt])
        .output()
        .await
        .context("failed to call claude for field naming")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("claude haiku failed: {stderr}"));
    }

    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();

    let json_str = extract_json_array(&raw).unwrap_or(&raw);

    let llm_fields: Vec<LlmField> = serde_json::from_str(json_str)
        .context("failed to parse LLM field naming response")?;

    let fields = llm_fields
        .into_iter()
        .map(|f| FieldDef {
            name: f.name,
            field_type: match f.field_type.as_str() {
                "url" => FieldType::Url,
                "number" => FieldType::Number,
                "dateTime" | "datetime" | "date" => FieldType::DateTime,
                _ => FieldType::Text,
            },
            source: FieldSource::Css(CssFieldSource {
                selector: f.css_selector,
                attribute: f.attribute,
            }),
        })
        .collect();

    Ok(fields)
}

fn infer_fields_heuristic(samples: &[String]) -> Vec<FieldDef> {
    let mut fields = Vec::new();

    let combined = samples.join(" ");
    let doc = Html::parse_fragment(&combined);

    if let Ok(sel) = Selector::parse("a[href]") {
        if doc.select(&sel).next().is_some() {
            fields.push(FieldDef {
                name: "title".into(),
                field_type: FieldType::Text,
                source: FieldSource::Css(CssFieldSource {
                    selector: "a".into(),
                    attribute: None,
                }),
            });
            fields.push(FieldDef {
                name: "url".into(),
                field_type: FieldType::Url,
                source: FieldSource::Css(CssFieldSource {
                    selector: "a".into(),
                    attribute: Some("href".into()),
                }),
            });
        }
    }

    fields
}

fn detect_pagination(html: &str) -> Option<PaginationDef> {
    let doc = Html::parse_document(html);

    let next_selectors = [
        ("a.morelink", "p"),
        ("a[rel=\"next\"]", "page"),
        ("a:contains(\"Next\")", "page"),
        ("a:contains(\"More\")", "p"),
    ];

    for (sel_str, param) in &next_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if doc.select(&sel).next().is_some() {
                return Some(PaginationDef {
                    strategy: PaginationStrategy::QueryParam,
                    next_css_selector: Some(sel_str.to_string()),
                    page_param: Some(param.to_string()),
                });
            }
        }
    }

    None
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

fn extract_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmField {
    name: String,
    #[serde(rename = "type", alias = "fieldType")]
    field_type: String,
    css_selector: String,
    #[serde(default)]
    attribute: Option<String>,
}
