use crate::resolve::ResolvedIrSource;
use anyhow::Context;
use std::path::{Path, PathBuf};

pub struct InstallPlan {
    pub source: ResolvedIrSource,
    pub descriptor: surfacer_ir::SiteDescriptor,
    pub site_home: PathBuf,
    pub shim_destination: PathBuf,
}

pub struct InstalledSite {
    pub site_name: String,
    pub ir_path: PathBuf,
    pub shim_path: PathBuf,
    pub command_count: usize,
}

pub fn lint_ir(ir_json: &surfacer_ir::SiteDescriptor) -> anyhow::Result<()> {
    surfacer_ir::lint_ir(ir_json).map_err(|errors| anyhow::anyhow!("{errors:?}"))
}

pub fn install_shim_to_path(
    shim: &Path,
    path_dir: &Path,
) -> anyhow::Result<PathBuf> {
    crate::shim::install_shim_to_path(shim, path_dir, file_name_string(shim)?.as_str())
}

pub fn register_site_locally(
    descriptor: &surfacer_ir::SiteDescriptor,
    ir_path: &Path,
    shim_path: &Path,
) -> anyhow::Result<InstalledSite> {
    Ok(InstalledSite {
        site_name: descriptor.meta.site_name.clone(),
        ir_path: ir_path.to_path_buf(),
        shim_path: shim_path.to_path_buf(),
        command_count: descriptor.operations.len(),
    })
}

pub fn install_site(
    ir: &surfacer_ir::SiteDescriptor,
    ir_source_path: &Path,
    home: &Path,
) -> anyhow::Result<InstalledSite> {
    lint_ir(ir)?;
    let site_name = &ir.meta.site_name;
    let site_dir = surfacer_ir::site_dir(home, site_name);
    std::fs::create_dir_all(&site_dir)
        .with_context(|| format!("failed to create {}", site_dir.display()))?;

    let installed_ir_path = surfacer_ir::site_ir_path(home, site_name);
    std::fs::copy(ir_source_path, &installed_ir_path).with_context(|| {
        format!(
            "failed to copy IR from {} to {}",
            ir_source_path.display(),
            installed_ir_path.display()
        )
    })?;

    let generated_shim_path = crate::shim::generate_shim(ir, site_name, &site_dir)?;
    let bin_dir = surfacer_ir::surfacer_home(home).join("bin");
    let installed_shim_path =
        crate::shim::install_shim_to_path(&generated_shim_path, &bin_dir, site_name)?;

    crate::registry::register_site(
        home,
        surfacer_ir::InstalledSiteEntry {
            site_name: site_name.clone(),
            ir_path: installed_ir_path.clone(),
            shim_path: installed_shim_path.clone(),
        },
    )?;

    Ok(InstalledSite {
        site_name: site_name.clone(),
        ir_path: installed_ir_path,
        shim_path: installed_shim_path,
        command_count: ir.operations.len(),
    })
}

fn file_name_string(path: &Path) -> anyhow::Result<String> {
    Ok(path
        .file_name()
        .and_then(|value| value.to_str())
        .context("shim path is missing a valid file name")?
        .to_string())
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
    #[ignore]
    fn test_install_site() {
        let dir = tempfile::tempdir().unwrap();
        let ir_path = dir.path().join("source-ir.json");
        let descriptor = sample_descriptor();
        surfacer_ir::write_ir(&ir_path, &descriptor).unwrap();

        let installed = install_site(&descriptor, &ir_path, dir.path()).unwrap();

        assert_eq!(installed.site_name, "sunat");
        assert!(installed.ir_path.exists());
        assert!(installed.shim_path.exists());
        assert_eq!(installed.command_count, 1);
    }
}
