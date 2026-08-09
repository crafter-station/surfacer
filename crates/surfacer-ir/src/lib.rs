pub mod ax;
pub mod descriptor;
pub mod extractor;
pub mod http;
pub mod lint;
pub mod meta;
pub mod paths;
pub mod registry;

pub use ax::*;
pub use descriptor::*;
pub use extractor::*;
pub use http::*;
pub use lint::*;
pub use meta::*;
pub use paths::*;
pub use registry::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_descriptor() -> SiteDescriptor {
        SiteDescriptor {
            meta: SiteMeta {
                site_name: "sunat".into(),
                display_name: "SUNAT Operaciones en Linea".into(),
                source_url: "https://www.sunat.gob.pe".into(),
                ir_version: "0.1.0".into(),
            },
            provenance: Provenance {
                generated_at: "2026-04-10T22:33:00Z".into(),
                technique: ProvenanceTechnique::Http,
                classifier_bucket: "FormSessionLegacy".into(),
                probe_duration_sec: 639,
            },
            operations: vec![
                OperationDescriptor {
                    command_path: vec!["rhe".into(), "consulta-emisor".into()],
                    summary: "Consulta RHE emitidos".into(),
                    description: "Consulta recibos por honorarios electronicos emitidos en un rango de fechas".into(),
                    operation_kind: OperationKind::Read,
                    transport: OperationTransport::Http(HttpOperation { endpoint_index: 0 }),
                    extractor: None,
                },
                OperationDescriptor {
                    command_path: vec!["ficha-ruc".into()],
                    summary: "Consulta ficha RUC".into(),
                    description: "Consulta la ficha RUC del contribuyente".into(),
                    operation_kind: OperationKind::Read,
                    transport: OperationTransport::Http(HttpOperation { endpoint_index: 1 }),
                    extractor: None,
                },
            ],
            http: Some(HttpSurface {
                endpoints: vec![
                    HttpEndpoint {
                        namespace: vec!["rhe".into()],
                        method: HttpMethod::Post,
                        path: "/ol-ti-itreciboelectronico/cpelec001Alias".into(),
                        description: "Consulta emisor de comprobantes electronicos".into(),
                        operation_kind: OperationKind::Read,
                        sample_request_content_type: Some("application/x-www-form-urlencoded".into()),
                        sample_response_content_type: Some("text/html".into()),
                        params: Vec::new(),
                    },
                    HttpEndpoint {
                        namespace: vec!["ruc".into()],
                        method: HttpMethod::Get,
                        path: "/cl-ti-itmrconsruc/consultaRuc".into(),
                        description: "Consulta ficha RUC".into(),
                        operation_kind: OperationKind::Read,
                        sample_request_content_type: None,
                        sample_response_content_type: Some("text/html".into()),
                        params: Vec::new(),
                    },
                ],
            }),
            ax: None,
        }
    }

    #[test]
    fn ir_roundtrips_json() {
        let descriptor = sample_descriptor();
        let json = serde_json::to_string_pretty(&descriptor).unwrap();
        let parsed: SiteDescriptor = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.meta.site_name, "sunat");
        assert_eq!(parsed.operations.len(), 2);
        assert_eq!(
            parsed.operations[0].command_path,
            vec!["rhe", "consulta-emisor"]
        );
        assert!(parsed.http.is_some());
        assert!(parsed.ax.is_none());
    }

    #[test]
    fn ir_writes_and_reads_from_disk() {
        let descriptor = sample_descriptor();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.surfacer.json");

        write_ir(&path, &descriptor).unwrap();
        assert!(path.exists());

        let loaded = read_ir(&path).unwrap();
        assert_eq!(loaded.meta.site_name, "sunat");
        assert_eq!(loaded.provenance.probe_duration_sec, 639);
        assert_eq!(loaded.operations.len(), 2);
    }

    #[test]
    fn ir_json_uses_camel_case() {
        let descriptor = sample_descriptor();
        let json = serde_json::to_string(&descriptor).unwrap();

        assert!(json.contains("siteName"));
        assert!(json.contains("sourceUrl"));
        assert!(json.contains("irVersion"));
        assert!(json.contains("commandPath"));
        assert!(json.contains("operationKind"));
        assert!(json.contains("endpointIndex"));
        assert!(!json.contains("site_name"));
        assert!(!json.contains("source_url"));
    }

    #[test]
    fn derive_operation_kind_from_method() {
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Get),
            OperationKind::Read
        ));
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Head),
            OperationKind::Read
        ));
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Options),
            OperationKind::Read
        ));
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Post),
            OperationKind::Write
        ));
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Put),
            OperationKind::Write
        ));
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Patch),
            OperationKind::Write
        ));
        assert!(matches!(
            derive_operation_kind(&HttpMethod::Delete),
            OperationKind::Write
        ));
    }

    #[test]
    fn camel_to_kebab_conversion() {
        assert_eq!(
            normalize_command_path(&["listPets".into(), "byStatus".into()]),
            vec!["list-pets", "by-status"]
        );
        assert_eq!(
            normalize_command_path(&["consulta".into()]),
            vec!["consulta"]
        );
        assert_eq!(normalize_command_path(&["RHE".into()]), vec!["r-h-e"]);
    }

    #[test]
    fn lint_catches_empty_site_name() {
        let mut descriptor = sample_descriptor();
        descriptor.meta.site_name = String::new();
        let errors = lint_ir(&descriptor).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, IrLintError::EmptySiteName))
        );
    }

    #[test]
    fn lint_catches_no_operations() {
        let mut descriptor = sample_descriptor();
        descriptor.operations.clear();
        let errors = lint_ir(&descriptor).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, IrLintError::NoOperations))
        );
    }

    #[test]
    fn lint_catches_duplicate_command_path() {
        let mut descriptor = sample_descriptor();
        descriptor.operations.push(descriptor.operations[0].clone());
        let errors = lint_ir(&descriptor).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, IrLintError::DuplicateCommandPath(_)))
        );
    }

    #[test]
    fn lint_passes_valid_ir() {
        let descriptor = sample_descriptor();
        assert!(lint_ir(&descriptor).is_ok());
    }

    #[test]
    fn command_help_rows_from_descriptor() {
        let descriptor = sample_descriptor();
        let rows = command_help_rows(&descriptor);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].command, "rhe consulta-emisor");
        assert_eq!(rows[1].command, "ficha-ruc");
    }

    #[test]
    fn registry_index_upsert_and_find() {
        let mut index = RegistryIndex { sites: Vec::new() };
        assert!(index.find("sunat").is_none());

        index.upsert(InstalledSiteEntry {
            site_name: "sunat".into(),
            ir_path: "/home/.surfacer/sites/sunat/ir.json".into(),
            shim_path: "/usr/local/bin/sunat".into(),
        });
        assert!(index.find("sunat").is_some());

        index.upsert(InstalledSiteEntry {
            site_name: "sunat".into(),
            ir_path: "/home/.surfacer/sites/sunat/ir-v2.json".into(),
            shim_path: "/usr/local/bin/sunat".into(),
        });
        assert_eq!(index.sites.len(), 1);
        assert!(
            index
                .find("sunat")
                .unwrap()
                .ir_path
                .to_str()
                .unwrap()
                .contains("v2")
        );
    }

    #[test]
    fn registry_index_remove() {
        let mut index = RegistryIndex {
            sites: vec![InstalledSiteEntry {
                site_name: "sunat".into(),
                ir_path: "/home/.surfacer/sites/sunat/ir.json".into(),
                shim_path: "/usr/local/bin/sunat".into(),
            }],
        };
        assert!(index.remove("sunat"));
        assert!(!index.remove("sunat"));
        assert!(index.sites.is_empty());
    }

    #[test]
    fn registry_roundtrips_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");

        let mut index = RegistryIndex { sites: Vec::new() };
        index.upsert(InstalledSiteEntry {
            site_name: "sunat".into(),
            ir_path: "sites/sunat/ir.json".into(),
            shim_path: "/usr/local/bin/sunat".into(),
        });

        index.save(&path).unwrap();
        let loaded = RegistryIndex::load(&path).unwrap();
        assert_eq!(loaded.sites.len(), 1);
        assert_eq!(loaded.sites[0].site_name, "sunat");
    }

    #[test]
    fn registry_load_returns_empty_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let index = RegistryIndex::load(&path).unwrap();
        assert!(index.sites.is_empty());
    }
}
