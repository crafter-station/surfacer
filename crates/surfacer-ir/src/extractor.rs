use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Extractor {
    List(ListExtractor),
    Detail(DetailExtractor),
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListExtractor {
    pub item_pattern: ItemPattern,
    pub fields: Vec<FieldDef>,
    #[serde(default)]
    pub pagination: Option<PaginationDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailExtractor {
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPattern {
    pub strategy: PatternStrategy,
    #[serde(default)]
    pub css_selector: Option<String>,
    #[serde(default)]
    pub ax_role: Option<String>,
    #[serde(default)]
    pub ax_name_pattern: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternStrategy {
    Css,
    AxTree,
    CssThenAx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub source: FieldSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldType {
    Text,
    Url,
    Number,
    DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "from")]
pub enum FieldSource {
    Css(CssFieldSource),
    AxTree(AxFieldSource),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssFieldSource {
    pub selector: String,
    #[serde(default)]
    pub attribute: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AxFieldSource {
    pub role: String,
    #[serde(default)]
    pub name_pattern: Option<String>,
    #[serde(default)]
    pub property: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationDef {
    pub strategy: PaginationStrategy,
    #[serde(default)]
    pub next_css_selector: Option<String>,
    #[serde(default)]
    pub page_param: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaginationStrategy {
    QueryParam,
    NextLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedItem {
    pub index: usize,
    pub fields: std::collections::BTreeMap<String, ExtractedValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "value")]
pub enum ExtractedValue {
    Text(String),
    Url(String),
    Number(f64),
    DateTime(String),
    Missing,
}

impl ExtractedItem {
    pub fn get_text(&self, name: &str) -> Option<&str> {
        match self.fields.get(name)? {
            ExtractedValue::Text(s) | ExtractedValue::Url(s) | ExtractedValue::DateTime(s) => {
                Some(s.as_str())
            }
            ExtractedValue::Number(_) | ExtractedValue::Missing => None,
        }
    }

    pub fn get_number(&self, name: &str) -> Option<f64> {
        match self.fields.get(name)? {
            ExtractedValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn get_url(&self, name: &str) -> Option<&str> {
        match self.fields.get(name)? {
            ExtractedValue::Url(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn primary_url(&self) -> Option<&str> {
        self.get_url("url")
            .or_else(|| self.get_url("link"))
            .or_else(|| self.get_url("href"))
            .or_else(|| self.get_url("commentsUrl"))
    }

    pub fn primary_title(&self) -> Option<&str> {
        self.get_text("title")
            .or_else(|| self.get_text("name"))
            .or_else(|| self.get_text("heading"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extractor_roundtrips_json() {
        let ext = Extractor::List(ListExtractor {
            item_pattern: ItemPattern {
                strategy: PatternStrategy::CssThenAx,
                css_selector: Some("tr.athing".into()),
                ax_role: Some("row".into()),
                ax_name_pattern: None,
            },
            fields: vec![
                FieldDef {
                    name: "title".into(),
                    field_type: FieldType::Text,
                    source: FieldSource::Css(CssFieldSource {
                        selector: "a".into(),
                        attribute: None,
                    }),
                },
                FieldDef {
                    name: "url".into(),
                    field_type: FieldType::Url,
                    source: FieldSource::Css(CssFieldSource {
                        selector: "a".into(),
                        attribute: Some("href".into()),
                    }),
                },
                FieldDef {
                    name: "points".into(),
                    field_type: FieldType::Number,
                    source: FieldSource::AxTree(AxFieldSource {
                        role: "text".into(),
                        name_pattern: Some(r"\d+ points".into()),
                        property: None,
                    }),
                },
            ],
            pagination: Some(PaginationDef {
                strategy: PaginationStrategy::QueryParam,
                next_css_selector: Some("a.morelink".into()),
                page_param: Some("p".into()),
            }),
        });

        let json = serde_json::to_string_pretty(&ext).unwrap();
        let parsed: Extractor = serde_json::from_str(&json).unwrap();

        match parsed {
            Extractor::List(list) => {
                assert_eq!(list.fields.len(), 3);
                assert_eq!(list.fields[0].name, "title");
                assert!(matches!(list.item_pattern.strategy, PatternStrategy::CssThenAx));
                assert!(list.pagination.is_some());
            }
            _ => panic!("expected List extractor"),
        }
    }

    #[test]
    fn detail_extractor_roundtrips() {
        let ext = Extractor::Detail(DetailExtractor {
            fields: vec![FieldDef {
                name: "content".into(),
                field_type: FieldType::Text,
                source: FieldSource::Css(CssFieldSource {
                    selector: "article".into(),
                    attribute: None,
                }),
            }],
        });

        let json = serde_json::to_string(&ext).unwrap();
        let parsed: Extractor = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Extractor::Detail(_)));
    }

    #[test]
    fn extracted_item_accessors() {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("title".into(), ExtractedValue::Text("Hello".into()));
        fields.insert("url".into(), ExtractedValue::Url("https://example.com".into()));
        fields.insert("points".into(), ExtractedValue::Number(42.0));

        let item = ExtractedItem { index: 1, fields };

        assert_eq!(item.get_text("title"), Some("Hello"));
        assert_eq!(item.get_url("url"), Some("https://example.com"));
        assert_eq!(item.get_number("points"), Some(42.0));
        assert_eq!(item.primary_url(), Some("https://example.com"));
        assert_eq!(item.primary_title(), Some("Hello"));
        assert_eq!(item.get_text("missing"), None);
    }

    #[test]
    fn raw_extractor_roundtrips() {
        let ext = Extractor::Raw;
        let json = serde_json::to_string(&ext).unwrap();
        let parsed: Extractor = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Extractor::Raw));
    }

    #[test]
    fn ax_field_source_roundtrips() {
        let field = FieldDef {
            name: "action_label".into(),
            field_type: FieldType::Text,
            source: FieldSource::AxTree(AxFieldSource {
                role: "button".into(),
                name_pattern: Some("Submit.*".into()),
                property: Some("name".into()),
            }),
        };

        let json = serde_json::to_string_pretty(&field).unwrap();
        assert!(json.contains("axTree"));
        assert!(json.contains("button"));

        let parsed: FieldDef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "action_label");
        match parsed.source {
            FieldSource::AxTree(ax) => {
                assert_eq!(ax.role, "button");
                assert_eq!(ax.name_pattern.as_deref(), Some("Submit.*"));
            }
            _ => panic!("expected AX source"),
        }
    }
}
