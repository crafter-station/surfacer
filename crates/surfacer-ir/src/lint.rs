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

pub fn command_help_rows(descriptor: &SiteDescriptor) -> Vec<CommandHelpRow> {
    descriptor
        .operations
        .iter()
        .map(|op| CommandHelpRow {
            command: op.command_path.join(" "),
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
