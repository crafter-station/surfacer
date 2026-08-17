use anyhow::Context;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::LintArgs;
use crate::output::{emit_json, use_color, Mode};

pub fn run(args: LintArgs) -> anyhow::Result<()> {
    let mode = Mode::resolve(args.json);

    let descriptor = surfacer_ir::read_ir(&args.ir_path)
        .with_context(|| format!("failed to read IR from {}", args.ir_path.display()))?;

    let read_ops = descriptor
        .operations
        .iter()
        .filter(|op| matches!(op.operation_kind, surfacer_ir::OperationKind::Read))
        .count();
    let write_ops = descriptor
        .operations
        .iter()
        .filter(|op| matches!(op.operation_kind, surfacer_ir::OperationKind::Write))
        .count();
    let http_count = descriptor
        .http
        .as_ref()
        .map(|h| h.endpoints.len())
        .unwrap_or(0);
    let ax_count = descriptor
        .ax
        .as_ref()
        .map(|a| a.actions.len())
        .unwrap_or(0);

    match surfacer_ir::lint_ir(&descriptor) {
        Ok(()) => {
            if mode.is_json() {
                // The endpoint count is the number a recon author checks
                // against the observed rows of their report, which is the only
                // automatic defense the observed-only rule has. It has to be
                // readable without parsing prose.
                emit_json(&json!({
                    "valid": true,
                    "site": descriptor.meta.site_name,
                    "displayName": descriptor.meta.display_name,
                    "irVersion": descriptor.meta.ir_version,
                    "technique": descriptor.provenance.technique,
                    "operations": {
                        "total": descriptor.operations.len(),
                        "read": read_ops,
                        "write": write_ops,
                    },
                    "endpoints": { "http": http_count, "ax": ax_count },
                    "errors": [],
                }));
                return Ok(());
            }

            if use_color() {
                eprintln!(
                    "{} {} {} ({})",
                    "✓".green(),
                    "Valid IR:".white(),
                    descriptor.meta.site_name.bold(),
                    descriptor.meta.display_name.dimmed()
                );
                eprintln!(
                    "  {} {} ({} read, {} write)",
                    "Operations:".dimmed(),
                    descriptor.operations.len().to_string().bold(),
                    read_ops.to_string().green(),
                    write_ops.to_string().yellow()
                );
                if http_count > 0 {
                    eprintln!(
                        "  {} {} HTTP",
                        "Endpoints: ".dimmed(),
                        http_count.to_string().cyan()
                    );
                }
                if ax_count > 0 {
                    eprintln!(
                        "  {} {} AX",
                        "Actions:   ".dimmed(),
                        ax_count.to_string().cyan()
                    );
                }
                eprintln!(
                    "  {} {:?}",
                    "Technique: ".dimmed(),
                    descriptor.provenance.technique
                );
                eprintln!(
                    "  {} {}",
                    "Version:   ".dimmed(),
                    descriptor.meta.ir_version.dimmed()
                );
            } else {
                eprintln!(
                    "✓ Valid IR: {} ({})",
                    descriptor.meta.site_name, descriptor.meta.display_name
                );
                eprintln!(
                    "  Operations: {} ({} read, {} write)",
                    descriptor.operations.len(),
                    read_ops,
                    write_ops
                );
                if http_count > 0 {
                    eprintln!("  Endpoints:  {http_count} HTTP");
                }
                if ax_count > 0 {
                    eprintln!("  Actions:    {ax_count} AX");
                }
                eprintln!("  Technique:  {:?}", descriptor.provenance.technique);
                eprintln!("  Version:    {}", descriptor.meta.ir_version);
            }
            Ok(())
        }
        Err(errors) => {
            if mode.is_json() {
                // A failed lint still answers on stdout, because "invalid, and
                // here is every reason" is the result the caller asked for.
                // The non-zero exit is what marks it as a failure.
                emit_json(&json!({
                    "valid": false,
                    "site": descriptor.meta.site_name,
                    "irVersion": descriptor.meta.ir_version,
                    "errors": errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
                }));
                return Err(anyhow::anyhow!("{} lint error(s) found", errors.len()));
            }

            if use_color() {
                eprintln!(
                    "{} {} {}",
                    "✗".red(),
                    "Invalid IR:".red(),
                    args.ir_path.display()
                );
                for error in &errors {
                    eprintln!("  {} {}", "·".red(), error);
                }
            } else {
                eprintln!("✗ Invalid IR: {}", args.ir_path.display());
                for error in &errors {
                    eprintln!("  - {error}");
                }
            }
            Err(anyhow::anyhow!("{} lint error(s) found", errors.len()))
        }
    }
}
