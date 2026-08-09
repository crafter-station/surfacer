use std::collections::HashSet;

use thiserror::Error;

use crate::{OperationDescriptor, OperationTransport, SiteDescriptor};

#[derive(Debug, Clone, Error)]
pub enum IrLintError {
    #[error("site_name is empty")]
    EmptySiteName,
    #[error("no operations defined")]
    NoOperations,
    #[error("operation at index {0} has empty command_path")]
    EmptyCommandPath(usize),
    #[error("duplicate command path: {0}")]
    DuplicateCommandPath(String),
    #[error("operation at index {0} has empty description")]
    EmptyDescription(usize),
    #[error("http surface declared but empty")]
    EmptyHttpSurface,
    #[error("http endpoint at index {0} has empty path")]
    EmptyEndpointPath(usize),
}

pub struct CommandHelpRow {
    pub command: String,
    pub description: String,
    /// Parameter names observed for this command during recon, most
    /// caller-controlled first. Empty when the descriptor predates parameters.
    pub params: Vec<String>,
}

pub fn lint_ir(descriptor: &SiteDescriptor) -> Result<(), Vec<IrLintError>> {
    let mut errors = Vec::new();

    if descriptor.meta.site_name.is_empty() {
        errors.push(IrLintError::EmptySiteName);
    }

    if descriptor.operations.is_empty() {
        errors.push(IrLintError::NoOperations);
    }

    let mut seen_paths: HashSet<String> = HashSet::new();

    for (i, op) in descriptor.operations.iter().enumerate() {
        if op.command_path.is_empty() {
            errors.push(IrLintError::EmptyCommandPath(i));
        } else {
            let key = op.command_path.join(" ");
            if !seen_paths.insert(key.clone()) {
                errors.push(IrLintError::DuplicateCommandPath(key));
            }
        }

        if op.description.is_empty() {
            errors.push(IrLintError::EmptyDescription(i));
        }
    }

    if let Some(ref http) = descriptor.http {
        if http.endpoints.is_empty() {
            errors.push(IrLintError::EmptyHttpSurface);
        }
        for (i, ep) in http.endpoints.iter().enumerate() {
            if ep.path.is_empty() {
                errors.push(IrLintError::EmptyEndpointPath(i));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Command names, made unique.
///
/// Recon derives a command from an endpoint path, and distinct endpoints can
/// normalize to the same one: SUNAT has seven operations under
/// `operatividadaduanera novedades`. Ambiguous names break anything that keys
/// on them, so duplicates past the first get a numeric suffix.
///
/// Every emitter should name operations through this rather than joining
/// `command_path` directly.
pub fn unique_command_names(descriptor: &SiteDescriptor) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    descriptor
        .operations
        .iter()
        .map(|op| {
            let base = op.command_path.join(" ");
            let count = seen.entry(base.clone()).or_insert(0);
            *count += 1;
            if *count == 1 {
                base
            } else {
                format!("{base} {}", *count)
            }
        })
        .collect()
}

pub fn command_help_rows(descriptor: &SiteDescriptor) -> Vec<CommandHelpRow> {
    let names = unique_command_names(descriptor);
    descriptor
        .operations
        .iter()
        .zip(names)
        .map(|(op, command)| CommandHelpRow {
            command,
            description: op.description.clone(),
            params: endpoint_for(descriptor, op)
                .map(|endpoint| endpoint.params.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default(),
        })
        .collect()
}

fn endpoint_for<'a>(
    descriptor: &'a SiteDescriptor,
    op: &OperationDescriptor,
) -> Option<&'a crate::HttpEndpoint> {
    match &op.transport {
        OperationTransport::Http(http) => descriptor
            .http
            .as_ref()
            .and_then(|surface| surface.endpoints.get(http.endpoint_index)),
        OperationTransport::Ax(_) => None,
    }
}
