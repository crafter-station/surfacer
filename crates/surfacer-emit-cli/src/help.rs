use owo_colors::OwoColorize;

pub fn build_help_text(descriptor: &surfacer_ir::SiteDescriptor) -> String {
    build_help_text_impl(descriptor, false)
}

pub fn build_help_text_colored(descriptor: &surfacer_ir::SiteDescriptor) -> String {
    build_help_text_impl(descriptor, true)
}

fn build_help_text_impl(descriptor: &surfacer_ir::SiteDescriptor, color: bool) -> String {
    let site = &descriptor.meta.site_name;
    let display = &descriptor.meta.display_name;
    let rows = surfacer_ir::command_help_rows(descriptor);
    let cmd_width = rows.iter().map(|r| r.command.len()).max().unwrap_or(0);

    let mut out = String::new();

    if color {
        out.push_str(&format!("{}\n\n", format!("{site} — {display}").bold()));
        out.push_str(&format!("{}\n", "USAGE".dimmed()));
        out.push_str(&format!("  {} {} {}\n\n", site.cyan(), "<command>".white(), "[flags]".dimmed()));
        out.push_str(&format!("{}\n", "COMMANDS".dimmed()));
    } else {
        out.push_str(&format!("{site} — {display}\n\n"));
        out.push_str("USAGE\n");
        out.push_str(&format!("  {site} <command> [flags]\n\n"));
        out.push_str("COMMANDS\n");
    }

    for row in &rows {
        if color {
            out.push_str(&format!("  {:width$}  {}\n",
                row.command.green(), row.description.dimmed(), width = cmd_width));
        } else {
            out.push_str(&format!("  {:width$}  {}\n",
                row.command, row.description, width = cmd_width));
        }
        if !row.params.is_empty() {
            let flags = row
                .params
                .iter()
                .map(|p| format!("{p}="))
                .collect::<Vec<_>>()
                .join(" ");
            let indent = " ".repeat(cmd_width + 4);
            if color {
                out.push_str(&format!("{}{}\n", indent, flags.dimmed()));
            } else {
                out.push_str(&format!("{indent}{flags}\n"));
            }
        }
    }
    out.push('\n');

    if color {
        out.push_str(&format!("{}\n", "FLAGS".dimmed()));
        out.push_str(&format!("  {}    {}\n", "--json".green(), "Output as JSON".dimmed()));
        out.push_str(&format!("  {}    {}\n\n", "--help".green(), "Show this help".dimmed()));
    } else {
        out.push_str("FLAGS\n");
        out.push_str("  --json    Output as JSON\n");
        out.push_str("  --help    Show this help\n\n");
    }

    let examples = pick_examples(site, &rows);
    if !examples.is_empty() {
        if color {
            out.push_str(&format!("{}\n", "TRY IT".dimmed()));
        } else {
            out.push_str("TRY IT\n");
        }
        for (cmd, desc) in &examples {
            if color {
                out.push_str(&format!("  {}  {}\n", cmd.cyan(), desc.dimmed()));
            } else {
                out.push_str(&format!("  {cmd}  {desc}\n"));
            }
        }
        out.push('\n');
    }

    if color {
        out.push_str(&format!("{}\n", "LEARN MORE".dimmed()));
        out.push_str(&format!("  {}    {}\n",
            format!("surfacer check {site}").cyan(), "Check for drift".dimmed()));
        out.push_str(&format!("  {}   {}\n",
            format!("surfacer update {site}").cyan(), "Update to latest IR".dimmed()));
    } else {
        out.push_str("LEARN MORE\n");
        out.push_str(&format!("  surfacer check {site}    Check for drift\n"));
        out.push_str(&format!("  surfacer update {site}   Update to latest IR\n"));
    }

    out
}

pub fn build_next_steps_after_exec(
    site: &str,
    current_command: &str,
    descriptor: &surfacer_ir::SiteDescriptor,
    color: bool,
) -> String {
    let rows = surfacer_ir::command_help_rows(descriptor);
    let other_commands: Vec<&surfacer_ir::CommandHelpRow> = rows
        .iter()
        .filter(|r| r.command != current_command)
        .collect();

    let mut out = String::new();

    if color {
        out.push_str(&format!("  {}\n", "Next:".dimmed()));
        out.push_str(&format!("    {}  {}\n",
            format!("{site} {current_command} --json").cyan(),
            "Machine-readable output".dimmed()));
    } else {
        out.push_str("  Next:\n");
        out.push_str(&format!("    {site} {current_command} --json  Machine-readable output\n"));
    }

    let suggestions: Vec<&&surfacer_ir::CommandHelpRow> = other_commands.iter().take(3).collect();
    if !suggestions.is_empty() {
        if color {
            out.push_str(&format!("  {}\n", "Other commands:".dimmed()));
        } else {
            out.push_str("  Other commands:\n");
        }
        for row in suggestions {
            if color {
                out.push_str(&format!("    {}  {}\n",
                    format!("{site} {}", row.command).cyan(),
                    row.description.dimmed()));
            } else {
                out.push_str(&format!("    {site} {}  {}\n", row.command, row.description));
            }
        }
    }

    out
}

fn pick_examples(site: &str, rows: &[surfacer_ir::CommandHelpRow]) -> Vec<(String, String)> {
    let mut examples = Vec::new();

    if let Some(first) = rows.first() {
        examples.push((
            format!("{site} {}", first.command),
            first.description.clone(),
        ));
    }

    if let Some(second) = rows.get(1) {
        examples.push((
            format!("{site} {} --json", second.command),
            format!("{} (JSON output)", second.description),
        ));
    }

    if rows.len() > 2 {
        examples.push((
            format!("{site} --help"),
            format!("See all {} commands", rows.len()),
        ));
    }

    examples
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_descriptor() -> surfacer_ir::SiteDescriptor {
        surfacer_ir::SiteDescriptor {
            meta: surfacer_ir::SiteMeta {
                site_name: "sunat".into(),
                display_name: "SUNAT Operaciones en Linea".into(),
                source_url: "https://www.sunat.gob.pe".into(),
                ir_version: "0.1.0".into(),
            },
            provenance: surfacer_ir::Provenance {
                generated_at: "2026-04-10T22:33:00Z".into(),
                technique: surfacer_ir::ProvenanceTechnique::Http,
                classifier_bucket: "FormSessionLegacy".into(),
                probe_duration_sec: 639,
            },
            operations: vec![
                surfacer_ir::OperationDescriptor {
                    command_path: vec!["rhe".into(), "consulta-emisor".into()],
                    summary: "Consulta RHE emitidos".into(),
                    description: "Consulta recibos por honorarios electronicos".into(),
                    operation_kind: surfacer_ir::OperationKind::Read,
                    transport: surfacer_ir::OperationTransport::Http(surfacer_ir::HttpOperation {
                        endpoint_index: 0,
                    }),
                    extractor: None,
                },
                surfacer_ir::OperationDescriptor {
                    command_path: vec!["ficha-ruc".into()],
                    summary: "Consulta ficha RUC".into(),
                    description: "Consulta la ficha RUC del contribuyente".into(),
                    operation_kind: surfacer_ir::OperationKind::Read,
                    transport: surfacer_ir::OperationTransport::Http(surfacer_ir::HttpOperation {
                        endpoint_index: 1,
                    }),
                    extractor: None,
                },
            ],
            http: Some(surfacer_ir::HttpSurface {
                endpoints: vec![
                    surfacer_ir::HttpEndpoint {
                        namespace: vec!["rhe".into()],
                        method: surfacer_ir::HttpMethod::Post,
                        path: "/ol-ti-itreciboelectronico/cpelec001Alias".into(),
                        description: "Consulta emisor".into(),
                        operation_kind: surfacer_ir::OperationKind::Read,
                        sample_request_content_type: Some("application/x-www-form-urlencoded".into()),
                        sample_response_content_type: Some("text/html".into()),
                        params: Vec::new(),
                    },
                    surfacer_ir::HttpEndpoint {
                        namespace: vec!["ruc".into()],
                        method: surfacer_ir::HttpMethod::Get,
                        path: "/cl-ti-itmrconsruc/consultaRuc".into(),
                        description: "Consulta ficha RUC".into(),
                        operation_kind: surfacer_ir::OperationKind::Read,
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
    fn help_has_try_it_section() {
        let help = build_help_text(&sample_descriptor());
        assert!(help.contains("TRY IT"));
        assert!(help.contains("sunat rhe consulta-emisor"));
        assert!(help.contains("sunat ficha-ruc --json"));
    }

    #[test]
    fn next_steps_shows_other_commands() {
        let d = sample_descriptor();
        let next = build_next_steps_after_exec("sunat", "rhe consulta-emisor", &d, false);
        assert!(next.contains("sunat rhe consulta-emisor --json"));
        assert!(next.contains("sunat ficha-ruc"));
        assert!(next.contains("Other commands:"));
    }
}
