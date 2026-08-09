use crate::resolve::ResolvedIrSource;

pub fn fetch_ir(source: &ResolvedIrSource) -> anyhow::Result<surfacer_ir::SiteDescriptor> {
    match source {
        ResolvedIrSource::LocalPath(path) => surfacer_ir::read_ir(path),
        ResolvedIrSource::GithubRepo(_) => Err(anyhow::anyhow!("GitHub install not yet supported")),
        ResolvedIrSource::RegistryName(_) => {
            Err(anyhow::anyhow!("Registry install not yet supported"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_descriptor() -> surfacer_ir::SiteDescriptor {
        surfacer_ir::SiteDescriptor {
            meta: surfacer_ir::SiteMeta {
                site_name: "sunat".into(),
                display_name: "SUNAT".into(),
                source_url: "https://example.com".into(),
                ir_version: "0.1.0".into(),
            },
            provenance: surfacer_ir::Provenance {
                generated_at: "2026-04-10T22:33:00Z".into(),
                technique: surfacer_ir::ProvenanceTechnique::Http,
                classifier_bucket: "FormSessionLegacy".into(),
                probe_duration_sec: 1,
            },
            operations: vec![surfacer_ir::OperationDescriptor {
                command_path: vec!["ficha-ruc".into()],
                summary: "Consulta ficha RUC".into(),
                description: "Consulta la ficha RUC".into(),
                operation_kind: surfacer_ir::OperationKind::Read,
                transport: surfacer_ir::OperationTransport::Http(surfacer_ir::HttpOperation {
                    endpoint_index: 0,
                }),
                    extractor: None,
                    auth: None,
            }],
            http: Some(surfacer_ir::HttpSurface {
                endpoints: vec![surfacer_ir::HttpEndpoint {
                    namespace: vec!["ruc".into()],
                    method: surfacer_ir::HttpMethod::Get,
                    path: "/consulta".into(),
                    description: "Consulta".into(),
                    operation_kind: surfacer_ir::OperationKind::Read,
                    sample_request_content_type: None,
                    sample_response_content_type: Some("application/json".into()),
                    params: Vec::new(),
                }],
                auth: None,
            }),
            ax: None,
        }
    }

    #[test]
    fn test_fetch_local_ir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ir.json");
        surfacer_ir::write_ir(&path, &sample_descriptor()).unwrap();

        let ir = fetch_ir(&ResolvedIrSource::LocalPath(path)).unwrap();

        assert_eq!(ir.meta.site_name, "sunat");
    }
}
