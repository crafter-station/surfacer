//! The shared TypeScript runtime, embedded into standalone emitters.
//!
//! # Why a shared runtime exists
//!
//! Generic execution logic used to live only in Rust: `extract.rs` applies an
//! IR's extractor to HTML, and `execute.rs` shapes the result. The shim target
//! inherited both by shelling out to `surfacer exec`. The standalone targets
//! inherited neither, so `ts-cli` returned raw HTML for a descriptor whose
//! extractor the Rust path used to return thirty structured items, and `mcp`
//! shipped without even the parameter check.
//!
//! That split was never declared: there are delegating emitters (a shim, which
//! needs surfacer installed) and standalone emitters (which need nothing, and
//! so must carry the logic). This module is what the standalone ones carry.
//! Written once per language, not once per emitter.
//!
//! # Why the source is inlined rather than emitted as sibling files
//!
//! Both shapes were measured on the committed HN descriptor. `scriptc coverage
//! --dynamic` reports 903 statements and 896 compiled statically (99%) for each
//! one, and the two binaries differ by 672 bytes out of 1.9MB, so neither
//! number discriminates.
//!
//! What discriminates is moving the emitted file. Copied on its own to another
//! directory, the sibling-file shape fails to compile because its imports no
//! longer resolve, while the inlined shape still reports 99% and builds. The
//! ts-cli target's whole claim is that it needs nothing else installed, and a
//! file that stops working when moved does not have that property. So: inline.

use surfacer_ir::{
    CssFieldSource, Extractor, FieldSource, FieldType, ItemPattern, PaginationDef,
    PaginationStrategy, PatternStrategy,
};

use crate::escaping::escape_ts;

/// The runtime modules, in dependency order.
///
/// `include_str!` keeps each one a real TypeScript file on disk: it type-checks
/// in an editor, runs under `bun` in the runtime's own tests, and cannot drift
/// from what the emitter ships, because the emitter ships exactly it.
const HTML_TS: &str = include_str!("../../../runtime/ts/html.ts");
const CSS_TS: &str = include_str!("../../../runtime/ts/css.ts");
const NORMALIZE_TS: &str = include_str!("../../../runtime/ts/normalize.ts");
const EXTRACT_TS: &str = include_str!("../../../runtime/ts/extract.ts");
const PARAMS_TS: &str = include_str!("../../../runtime/ts/params.ts");
const EXEC_TS: &str = include_str!("../../../runtime/ts/exec.ts");

/// The runtime modules in the order they must appear when concatenated.
const MODULES: [(&str, &str); 6] = [
    ("html.ts", HTML_TS),
    ("css.ts", CSS_TS),
    ("normalize.ts", NORMALIZE_TS),
    ("extract.ts", EXTRACT_TS),
    ("params.ts", PARAMS_TS),
    ("exec.ts", EXEC_TS),
];

/// The runtime as one inlinable TypeScript blob.
///
/// Two transforms are applied while concatenating, both required because the
/// result is a single module rather than six:
///
/// 1. Intra-runtime imports are dropped. Every symbol they name is in the same
///    file after concatenation, so the import would reference a module that no
///    longer exists.
/// 2. `export` is stripped from declarations. A file with no import and no
///    export is a script, which is what the emitted CLI is; leaving the keyword
///    would make the emitted program a module and change how the entry point
///    runs.
///
/// Imports of `node:*` are preserved, because those still resolve.
pub fn runtime_source() -> String {
    let mut out = String::new();

    out.push_str(
        "// ---------------------------------------------------------------------------\n\
         // surfacer TypeScript runtime, inlined.\n\
         //\n\
         // Source of truth: runtime/ts/*.ts in the surfacer repository. This copy is\n\
         // embedded by the emitter, so editing it here is pointless: regenerating the\n\
         // file overwrites the edit. Fix the runtime and re-emit.\n\
         //\n\
         // It is inlined rather than imported so the emitted program keeps working when\n\
         // it is copied somewhere else on its own, which is the whole promise of the\n\
         // standalone targets.\n\
         // ---------------------------------------------------------------------------\n\n",
    );

    for (name, source) in MODULES {
        out.push_str(&format!("// ===== runtime/ts/{name} =====\n"));
        out.push_str(&strip_module_syntax(source));
        out.push('\n');
    }

    out
}

/// Drop intra-runtime imports and leading `export` keywords.
///
/// Deliberately line-oriented rather than a TypeScript parse: every import in
/// the runtime is written on a single line, and the runtime is the only input
/// this ever sees. A test asserts the emitted blob has no `from "./"` left, so
/// a future multi-line import fails loudly instead of shipping broken.
fn strip_module_syntax(source: &str) -> String {
    let mut out = String::new();
    let mut in_local_import = false;

    for line in source.lines() {
        let trimmed = line.trim_start();

        // A local import may span lines when the named list is long.
        if in_local_import {
            if trimmed.contains("from \"./") || trimmed.contains("from './") {
                in_local_import = false;
            }
            continue;
        }
        if trimmed.starts_with("import ") || trimmed.starts_with("import{") {
            if trimmed.contains("from \"./") || trimmed.contains("from './") {
                continue;
            }
            if !trimmed.contains(" from ") {
                // The opening line of a multi-line import; the module it names
                // is on a later line, so decide there.
                in_local_import = true;
                continue;
            }
            // A `node:` import survives: it still resolves in the single file.
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("export ") {
            let indent = &line[..line.len() - trimmed.len()];
            out.push_str(indent);
            out.push_str(rest);
            out.push('\n');
            continue;
        }

        out.push_str(line);
        out.push('\n');
    }

    out
}

/// Render an `Extractor` as a TypeScript literal the runtime accepts.
///
/// Every field is written explicitly, including the empty ones, and the literal
/// is annotated `: Extractor` at the use site. Both are required rather than
/// stylistic: scriptc matches record types exactly, so an object that omits a
/// field, or that writes `null` where a sibling writes a string, fails to
/// compile with SC2002/SC2003. The IR's `Option<String>` becomes `""`, which
/// the runtime reads as absent.
pub fn render_extractor(extractor: &Extractor) -> String {
    match extractor {
        Extractor::List(list) => format!(
            "{{\n    type: \"list\",\n    itemPattern: {},\n    fields: [{}],\n    pagination: {},\n  }}",
            render_item_pattern(&list.item_pattern),
            render_fields(&list.fields),
            render_pagination(list.pagination.as_ref()),
        ),
        Extractor::Detail(detail) => format!(
            "{{\n    type: \"detail\",\n    itemPattern: {},\n    fields: [{}],\n    pagination: {},\n  }}",
            empty_item_pattern(),
            render_fields(&detail.fields),
            render_pagination(None),
        ),
        Extractor::Raw => format!(
            "{{\n    type: \"raw\",\n    itemPattern: {},\n    fields: [],\n    pagination: {},\n  }}",
            empty_item_pattern(),
            render_pagination(None),
        ),
    }
}

fn render_item_pattern(pattern: &ItemPattern) -> String {
    let strategy = match pattern.strategy {
        PatternStrategy::Css => "css",
        PatternStrategy::AxTree => "axTree",
        PatternStrategy::CssThenAx => "cssThenAx",
    };
    format!(
        "{{ strategy: \"{strategy}\", cssSelector: \"{}\", axRole: \"{}\", axNamePattern: \"{}\" }}",
        escape_ts(pattern.css_selector.as_deref().unwrap_or("")),
        escape_ts(pattern.ax_role.as_deref().unwrap_or("")),
        escape_ts(pattern.ax_name_pattern.as_deref().unwrap_or("")),
    )
}

fn empty_item_pattern() -> String {
    "{ strategy: \"css\", cssSelector: \"\", axRole: \"\", axNamePattern: \"\" }".to_string()
}

fn render_fields(fields: &[surfacer_ir::FieldDef]) -> String {
    if fields.is_empty() {
        return String::new();
    }
    let rendered = fields
        .iter()
        .map(|field| {
            let field_type = match field.field_type {
                FieldType::Text => "text",
                FieldType::Url => "url",
                FieldType::Number => "number",
                FieldType::DateTime => "dateTime",
            };
            format!(
                "\n      {{ name: \"{}\", fieldType: \"{field_type}\", source: {} }},",
                escape_ts(&field.name),
                render_field_source(&field.source),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("{rendered}\n    ")
}

/// A field source, flattened so both variants share one record shape.
///
/// The runtime models this as a single record with a `from` discriminant rather
/// than a union, because an array holding one CSS source and one AX source does
/// not unify under scriptc's exact-match rule. The inapplicable half is empty.
fn render_field_source(source: &FieldSource) -> String {
    match source {
        FieldSource::Css(CssFieldSource {
            selector,
            attribute,
        }) => format!(
            "{{ from: \"css\", selector: \"{}\", attribute: \"{}\", role: \"\", namePattern: \"\", property: \"\" }}",
            escape_ts(selector),
            escape_ts(attribute.as_deref().unwrap_or("")),
        ),
        FieldSource::AxTree(ax) => format!(
            "{{ from: \"axTree\", selector: \"\", attribute: \"\", role: \"{}\", namePattern: \"{}\", property: \"{}\" }}",
            escape_ts(&ax.role),
            escape_ts(ax.name_pattern.as_deref().unwrap_or("")),
            escape_ts(ax.property.as_deref().unwrap_or("")),
        ),
    }
}

/// Pagination, with a default that is inert rather than absent.
///
/// A missing pagination block becomes a queryParam strategy with empty fields,
/// which the runtime never acts on because it checks the fields, not the
/// presence of the record. Modelling absence as `undefined` would reintroduce
/// the optional type the record-exactness rule rejects.
fn render_pagination(pagination: Option<&PaginationDef>) -> String {
    match pagination {
        None => {
            "{ strategy: \"queryParam\", nextCssSelector: \"\", pageParam: \"\" }".to_string()
        }
        Some(def) => {
            let strategy = match def.strategy {
                PaginationStrategy::QueryParam => "queryParam",
                PaginationStrategy::NextLink => "nextLink",
            };
            format!(
                "{{ strategy: \"{strategy}\", nextCssSelector: \"{}\", pageParam: \"{}\" }}",
                escape_ts(def.next_css_selector.as_deref().unwrap_or("")),
                escape_ts(def.page_param.as_deref().unwrap_or("")),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_source_carries_every_module() {
        let source = runtime_source();
        for (name, _) in MODULES {
            assert!(
                source.contains(&format!("runtime/ts/{name}")),
                "inlined runtime is missing {name}"
            );
        }
    }

    #[test]
    fn runtime_source_has_no_local_imports_left() {
        // A surviving relative import would make the emitted single file
        // depend on siblings that are never written, which is the exact
        // property the inline shape was chosen to avoid.
        let source = runtime_source();
        assert!(!source.contains("from \"./"), "local import survived inlining");
        assert!(!source.contains("from './"), "local import survived inlining");
    }

    #[test]
    fn runtime_source_keeps_node_imports() {
        // params.ts and exec.ts have none today, but html.ts must never lose a
        // `node:` import if one is added: those still resolve in a single file.
        let source = runtime_source();
        for line in source.lines() {
            if line.trim_start().starts_with("import ") {
                assert!(
                    line.contains("node:"),
                    "unexpected non-node import survived: {line}"
                );
            }
        }
    }

    #[test]
    fn runtime_source_strips_export_keyword() {
        let source = runtime_source();
        for line in source.lines() {
            assert!(
                !line.trim_start().starts_with("export "),
                "export survived inlining: {line}"
            );
        }
    }

    #[test]
    fn list_extractor_renders_every_field_explicitly() {
        use surfacer_ir::{FieldDef, ListExtractor};

        let extractor = Extractor::List(ListExtractor {
            item_pattern: ItemPattern {
                strategy: PatternStrategy::Css,
                css_selector: Some("tr.athing".into()),
                ax_role: None,
                ax_name_pattern: None,
            },
            fields: vec![FieldDef {
                name: "title".into(),
                field_type: FieldType::Text,
                source: FieldSource::Css(CssFieldSource {
                    selector: "a".into(),
                    attribute: None,
                }),
            }],
            pagination: None,
        });

        let rendered = render_extractor(&extractor);

        // Absence is the empty string, never `null` or an omitted key: scriptc
        // rejects a record that does not match its expected shape exactly.
        assert!(rendered.contains("axRole: \"\""));
        assert!(rendered.contains("attribute: \"\""));
        assert!(rendered.contains("pagination: { strategy: \"queryParam\""));
        assert!(!rendered.contains("null"));
        assert!(!rendered.contains("undefined"));
    }

    #[test]
    fn ax_field_source_fills_only_the_ax_half() {
        use surfacer_ir::AxFieldSource;

        let source = FieldSource::AxTree(AxFieldSource {
            role: "button".into(),
            name_pattern: Some("Submit.*".into()),
            property: None,
        });

        let rendered = render_field_source(&source);
        assert!(rendered.contains("from: \"axTree\""));
        assert!(rendered.contains("role: \"button\""));
        assert!(rendered.contains("selector: \"\""));
        assert!(rendered.contains("property: \"\""));
    }

    #[test]
    fn selector_with_quotes_is_escaped() {
        // The committed HN descriptor uses `a[href^="https://"]`, so an
        // unescaped quote here would break every emitted CLI for that site.
        let pattern = ItemPattern {
            strategy: PatternStrategy::Css,
            css_selector: Some(r#"tr:has(> td > span > a[href^="https://"])"#.into()),
            ax_role: None,
            ax_name_pattern: None,
        };
        let rendered = render_item_pattern(&pattern);
        assert!(rendered.contains(r#"a[href^=\"https://\"]"#));
    }
}
