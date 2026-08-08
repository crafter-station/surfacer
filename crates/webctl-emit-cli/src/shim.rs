use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow};

use crate::manifest::{ShimManifest, write_manifest};
use crate::template::{shim_cargo_toml, shim_main_rs};

pub struct CliEmitRequest {
    pub descriptor: webctl_ir::SiteDescriptor,
    pub out_dir: std::path::PathBuf,
}

pub struct EmittedShim {
    pub manifest: ShimManifest,
    pub project_dir: std::path::PathBuf,
    pub binary_path: std::path::PathBuf,
    pub binary_size: u64,
}

pub fn emit_cli_shim(request: CliEmitRequest) -> anyhow::Result<EmittedShim> {
    let site_name = request.descriptor.meta.site_name.clone();
    let out_dir = request.out_dir;
    let webctl_binary = PathBuf::from("webctl");
    let project_dir = out_dir.clone();
    let binary_path = out_dir.join(&site_name);
    let manifest_path = out_dir.join("shim-manifest.json");

    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create output dir {}", out_dir.display()))?;

    let temp_dir = build_temp_dir(&site_name)?;
    std::fs::create_dir_all(&temp_dir)
        .with_context(|| format!("failed to create temp dir {}", temp_dir.display()))?;

    let source_path = temp_dir.join("main.rs");
    let cargo_toml_path = temp_dir.join("Cargo.toml");
    let compiled_binary_path = temp_dir.join(&site_name);

    std::fs::write(
        &source_path,
        shim_main_rs(&site_name, &webctl_binary.to_string_lossy()),
    )
    .with_context(|| format!("failed to write {}", source_path.display()))?;
    std::fs::write(&cargo_toml_path, shim_cargo_toml(&site_name))
        .with_context(|| format!("failed to write {}", cargo_toml_path.display()))?;

    compile_shim(&source_path, &compiled_binary_path)?;
    maybe_strip_binary(&compiled_binary_path);

    std::fs::copy(&compiled_binary_path, &binary_path).with_context(|| {
        format!(
            "failed to copy compiled shim from {} to {}",
            compiled_binary_path.display(),
            binary_path.display()
        )
    })?;

    let compiled_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock before unix epoch: {err}"))?
        .as_secs()
        .to_string();
    let manifest = ShimManifest {
        site_name,
        webctl_path: webctl_binary,
        shim_path: binary_path.clone(),
        compiled_at,
    };
    write_manifest(&manifest_path, &manifest)?;

    let metadata = std::fs::metadata(&binary_path)
        .with_context(|| format!("failed to stat {}", binary_path.display()))?;
    let _ = std::fs::remove_dir_all(&temp_dir);

    Ok(EmittedShim {
        manifest,
        project_dir,
        binary_path,
        binary_size: metadata.len(),
    })
}

fn build_temp_dir(site_name: &str) -> anyhow::Result<PathBuf> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| anyhow!("system clock before unix epoch: {err}"))?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "webctl-emit-cli-{site_name}-{}-{unique}",
        std::process::id()
    )))
}

fn compile_shim(source_path: &Path, output_path: &Path) -> anyhow::Result<()> {
    let status = Command::new("rustc")
        .arg(source_path)
        .arg("-O")
        .arg("-C")
        .arg("panic=abort")
        .arg("-C")
        .arg("opt-level=z")
        .arg("-C")
        .arg("codegen-units=1")
        .arg("-o")
        .arg(output_path)
        .status()
        .with_context(|| "failed to invoke rustc")?;

    if !status.success() {
        return Err(anyhow!("rustc failed with status {status}"));
    }

    Ok(())
}

fn maybe_strip_binary(path: &Path) {
    let _ = Command::new("strip").arg(path).status();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_descriptor() -> webctl_ir::SiteDescriptor {
        webctl_ir::SiteDescriptor {
            meta: webctl_ir::SiteMeta {
                site_name: "sunat".into(),
                display_name: "SUNAT Operaciones en Linea".into(),
                source_url: "https://www.sunat.gob.pe".into(),
                ir_version: "0.1.0".into(),
            },
            provenance: webctl_ir::Provenance {
                generated_at: "2026-04-10T22:33:00Z".into(),
                technique: webctl_ir::ProvenanceTechnique::Http,
                classifier_bucket: "FormSessionLegacy".into(),
                probe_duration_sec: 639,
            },
            operations: vec![webctl_ir::OperationDescriptor {
                command_path: vec!["ficha-ruc".into()],
                summary: "Consulta ficha RUC".into(),
                description: "Consulta la ficha RUC del contribuyente".into(),
                operation_kind: webctl_ir::OperationKind::Read,
                transport: webctl_ir::OperationTransport::Http(webctl_ir::HttpOperation {
                    endpoint_index: 0,
                }),
                    extractor: None,
            }],
            http: Some(webctl_ir::HttpSurface {
                endpoints: vec![webctl_ir::HttpEndpoint {
                    namespace: vec!["ruc".into()],
                    method: webctl_ir::HttpMethod::Get,
                    path: "/cl-ti-itmrconsruc/consultaRuc".into(),
                    description: "Consulta ficha RUC".into(),
                    operation_kind: webctl_ir::OperationKind::Read,
                    sample_request_content_type: None,
                    sample_response_content_type: Some("text/html".into()),
                    params: Vec::new(),
                }],
            }),
            ax: None,
        }
    }

    #[test]
    fn test_shim_compilation() {
        if Command::new("rustc").arg("--version").status().is_err() {
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let emitted = emit_cli_shim(CliEmitRequest {
            descriptor: sample_descriptor(),
            out_dir: dir.path().to_path_buf(),
        })
        .unwrap();

        assert!(emitted.binary_path.exists());
        assert!(emitted.binary_size < 500 * 1024);
    }
}
