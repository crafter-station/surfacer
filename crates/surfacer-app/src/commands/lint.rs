use std::io::IsTerminal;

use anyhow::Context;
use owo_colors::OwoColorize;

use crate::cli::LintArgs;

fn color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal()
}

pub fn run(args: LintArgs) -> anyhow::Result<()> {
    let descriptor = surfacer_ir::read_ir(&args.ir_path)
        .with_context(|| format!("failed to read IR from {}", args.ir_path.display()))?;

    match surfacer_ir::lint_ir(&descriptor) {
        Ok(()) => {
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

            if color() {
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
                    eprintln!("  Endpoints:  {} HTTP", http_count);
                }
                if ax_count > 0 {
                    eprintln!("  Actions:    {} AX", ax_count);
                }
                eprintln!("  Technique:  {:?}", descriptor.provenance.technique);
                eprintln!("  Version:    {}", descriptor.meta.ir_version);
            }
            Ok(())
        }
        Err(errors) => {
            if color() {
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
