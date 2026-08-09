use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};

pub fn generate_shim(
    ir: &surfacer_ir::SiteDescriptor,
    site_name: &str,
    destination_dir: &Path,
) -> anyhow::Result<PathBuf> {
    ensure!(
        ir.meta.site_name == site_name,
        "site name does not match descriptor"
    );
    let emitted = surfacer_emit_cli::emit_cli_shim(surfacer_emit_cli::CliEmitRequest {
        descriptor: ir.clone(),
        out_dir: destination_dir.to_path_buf(),
    })?;
    Ok(emitted.binary_path)
}

pub fn install_shim_to_path(
    shim_path: &Path,
    bin_dir: &Path,
    site_name: &str,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(bin_dir)
        .with_context(|| format!("failed to create {}", bin_dir.display()))?;
    let installed_path = bin_dir.join(site_name);
    std::fs::copy(shim_path, &installed_path).with_context(|| {
        format!(
            "failed to copy shim from {} to {}",
            shim_path.display(),
            installed_path.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = std::fs::metadata(&installed_path)?.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&installed_path, permissions)?;
    }
    Ok(installed_path)
}
