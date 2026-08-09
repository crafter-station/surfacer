use std::collections::BTreeMap;

use scraper::{Html, Selector};
use surfacer_ir::{
    CssFieldSource, ExtractedItem, ExtractedValue, Extractor, FieldDef, FieldSource, FieldType,
    ListExtractor,
};

pub fn extract_items(html: &str, extractor: &Extractor) -> Option<Vec<ExtractedItem>> {
    match extractor {
        Extractor::List(list) => {
            let result = extract_list(html, list);
            if result.is_some() {
                return result;
            }
            extract_list_with_siblings(html, list)
        }
        Extractor::Detail(_detail) => None,
        Extractor::Raw => None,
    }
}

fn extract_list(html: &str, list: &ListExtractor) -> Option<Vec<ExtractedItem>> {
    let css = list.item_pattern.css_selector.as_deref()?;
    let doc = Html::parse_document(html);
    let item_sel = Selector::parse(css).ok()?;

    let items: Vec<ExtractedItem> = doc
        .select(&item_sel)
        .enumerate()
        .map(|(i, element)| {
            let mut fields = BTreeMap::new();

            for field_def in &list.fields {
                let value = extract_field(&doc, &element, field_def);
                fields.insert(field_def.name.clone(), value);
            }

            ExtractedItem {
                index: i + 1,
                fields,
            }
        })
        .filter(|item| {
            item.fields.values().any(|v| !matches!(v, ExtractedValue::Missing))
        })
        .collect();

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn extract_list_with_siblings(html: &str, list: &ListExtractor) -> Option<Vec<ExtractedItem>> {
    let css = list.item_pattern.css_selector.as_deref()?;
    let doc = Html::parse_document(html);
    let item_sel = Selector::parse(css).ok()?;
    let all_trs = Selector::parse("tr").ok()?;

    let all_rows: Vec<scraper::ElementRef> = doc.select(&all_trs).collect();
    let matched_indices: Vec<usize> = all_rows
        .iter()
        .enumerate()
        .filter(|(_, el)| item_sel.matches(el))
        .map(|(i, _)| i)
        .collect();

    if matched_indices.is_empty() {
        return None;
    }

    let items: Vec<ExtractedItem> = matched_indices
        .iter()
        .enumerate()
        .filter_map(|(item_idx, &row_idx)| {
            let element = &all_rows[row_idx];
            let mut fields = BTreeMap::new();

            for field_def in &list.fields {
                let value = extract_field(&doc, element, field_def);
                if !matches!(value, ExtractedValue::Missing) {
                    fields.insert(field_def.name.clone(), value);
                    continue;
                }

                if let Some(sibling) = all_rows.get(row_idx + 1) {
                    let sibling_value = extract_field(&doc, sibling, field_def);
                    if !matches!(sibling_value, ExtractedValue::Missing) {
                        fields.insert(field_def.name.clone(), sibling_value);
                        continue;
                    }
                }

                let combined_text = element.text().collect::<Vec<_>>().join(" ");
                let sibling_text = all_rows
                    .get(row_idx + 1)
                    .map(|s| s.text().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default();
                let full_text = format!("{combined_text} {sibling_text}");

                let text_value = extract_from_text(&full_text, &field_def.name, &field_def.field_type);
                fields.insert(field_def.name.clone(), text_value);
            }

            if fields.values().all(|v| matches!(v, ExtractedValue::Missing)) {
                return None;
            }

            Some(ExtractedItem {
                index: item_idx + 1,
                fields,
            })
        })
        .collect();

    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn extract_from_text(text: &str, field_name: &str, field_type: &FieldType) -> ExtractedValue {
    match field_name {
        "points" => {
            let re = regex::Regex::new(r"(\d+)\s*points?").ok();
            re.and_then(|r| r.captures(text))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .map(ExtractedValue::Number)
                .unwrap_or(ExtractedValue::Missing)
        }
        "author" => {
            let re = regex::Regex::new(r"by\s+(\w+)").ok();
            re.and_then(|r| r.captures(text))
                .and_then(|c| c.get(1))
                .map(|m| ExtractedValue::Text(m.as_str().to_string()))
                .unwrap_or(ExtractedValue::Missing)
        }
        "age" | "time" => {
            let re = regex::Regex::new(r"(\d+\s*(?:minutes?|hours?|days?|months?|years?)\s*ago)").ok();
            re.and_then(|r| r.find(text))
                .map(|m| ExtractedValue::Text(m.as_str().to_string()))
                .unwrap_or(ExtractedValue::Missing)
        }
        "comments" | "commentCount" => {
            let re = regex::Regex::new(r"(\d+)\s*comments?").ok();
            re.and_then(|r| r.captures(text))
                .and_then(|c| c.get(1))
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .map(ExtractedValue::Number)
                .unwrap_or(ExtractedValue::Missing)
        }
        _ => {
            match field_type {
                FieldType::Number => ExtractedValue::Missing,
                _ => ExtractedValue::Missing,
            }
        }
    }
}

fn extract_field(
    _doc: &Html,
    element: &scraper::ElementRef,
    field_def: &FieldDef,
) -> ExtractedValue {
    match &field_def.source {
        FieldSource::Css(css) => extract_css_field(element, css, &field_def.field_type),
        FieldSource::AxTree(_) => ExtractedValue::Missing,
    }
}

fn extract_css_field(
    element: &scraper::ElementRef,
    source: &CssFieldSource,
    field_type: &FieldType,
) -> ExtractedValue {
    let sel = match Selector::parse(&source.selector) {
        Ok(s) => s,
        Err(_) => return ExtractedValue::Missing,
    };

    let target = element.select(&sel).next();
    let Some(target) = target else {
        return ExtractedValue::Missing;
    };

    let raw = if let Some(ref attr) = source.attribute {
        target.value().attr(attr).map(|s| s.to_string())
    } else {
        Some(target.text().collect::<Vec<_>>().join("").trim().to_string())
    };

    let Some(raw) = raw else {
        return ExtractedValue::Missing;
    };

    if raw.is_empty() {
        return ExtractedValue::Missing;
    }

    match field_type {
        FieldType::Text | FieldType::DateTime => ExtractedValue::Text(raw),
        FieldType::Url => ExtractedValue::Url(raw),
        FieldType::Number => {
            let digits: String = raw.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            digits
                .parse::<f64>()
                .map(ExtractedValue::Number)
                .unwrap_or(ExtractedValue::Text(raw))
        }
    }
}
