use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};
use url::Url;
use webctl_classifier::ax_stub::extract_ax_actions;
use webctl_classifier::http_infer::infer_endpoints;

use crate::commands::{emit, install, lint, recon};
use crate::ui::prompt;

fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal()
}

fn ok(msg: &str) {
    if use_color() {
        eprintln!("{} {}", "✓".green(), msg);
    } else {
        eprintln!("✓ {msg}");
    }
}

fn step(msg: &str) {
    if use_color() {
        eprintln!("{} {}", "⠋".cyan(), msg);
    } else {
        eprintln!("⠋ {msg}");
    }
}

fn warn(msg: &str) {
    if use_color() {
        eprintln!("{} {}", "⚠".yellow(), msg);
    } else {
        eprintln!("⚠ {msg}");
    }
}

fn err(msg: &str) {
    if use_color() {
        eprintln!("{} {}", "✗".red(), msg);
    } else {
        eprintln!("✗ {msg}");
    }
}

fn hint(msg: &str) {
    if use_color() {
        eprintln!("  {}", msg.dimmed());
    } else {
        eprintln!("  {msg}");
    }
}

fn cmd(msg: &str) {
    if use_color() {
        eprintln!("    {}", msg.cyan());
    } else {
        eprintln!("    {msg}");
    }
}

#[derive(Debug, Parser)]
#[command(
    name = "webctl",
    about = "CLI-ify the web — turn any website into an installable CLI",
    after_help = "Getting started:\n  webctl recon <url> --auto --yes\n\nLearn more:\n  https://github.com/crafter-station/webctl"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Recon(ReconArgs),
    Emit(EmitArgs),
    Install(InstallArgs),
    Lint(LintArgs),
    Exec(ExecArgs),
    Auth(AuthArgs),
    Check(CheckArgs),
    Shell,
}

#[derive(Debug, Clone, clap::Args)]
pub struct CheckArgs {
    pub site: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub action: AuthAction,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AuthAction {
    Login(AuthLoginArgs),
    Status(AuthStatusArgs),
    Logout(AuthLogoutArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct AuthLoginArgs {
    pub site: String,
    #[arg(long)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct AuthStatusArgs {
    pub site: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct AuthLogoutArgs {
    pub site: String,
}

#[derive(Debug, Clone, clap::Args)]
#[command(disable_help_flag = true)]
pub struct ExecArgs {
    pub site: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct ReconArgs {
    pub url: String,

    #[arg(long, value_enum, default_value = "read-only")]
    pub policy: ProbePolicy,

    #[arg(long)]
    pub yes: bool,

    #[arg(long)]
    pub auto: bool,

    #[arg(long, group = "technique")]
    pub http: bool,

    #[arg(long, group = "technique")]
    pub ax: bool,

    #[arg(long, group = "technique")]
    pub hybrid: bool,

    #[arg(long)]
    pub output: Option<PathBuf>,
}

impl ReconArgs {
    pub fn technique_override(&self) -> Option<TechniqueOverride> {
        if self.http {
            Some(TechniqueOverride::Http)
        } else if self.ax {
            Some(TechniqueOverride::Ax)
        } else if self.hybrid {
            Some(TechniqueOverride::Hybrid)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ProbePolicy {
    ReadOnly,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TechniqueOverride {
    Http,
    Ax,
    Hybrid,
}

#[derive(Debug, Clone, clap::Args)]
pub struct InstallArgs {
    pub source: String,

    #[arg(long = "dest")]
    pub dest: Option<PathBuf>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct EmitArgs {
    #[command(subcommand)]
    pub target: EmitTargetArg,

    #[arg(long = "out-dir")]
    pub out_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum EmitTargetArg {
    Cli { ir_path: PathBuf },
    JustBash { ir_path: PathBuf },
}

#[derive(Debug, Clone, clap::Args)]
pub struct LintArgs {
    pub ir_path: PathBuf,
}

#[derive(Debug)]
pub struct ClassifierSuggestionView {
    pub bucket: webctl_classifier::ClassifierBucket,
    pub confidence: webctl_classifier::Confidence,
    pub http_endpoint_count: usize,
    pub ax_action_count: usize,
}

#[derive(Debug)]
pub struct InstallProgressView {
    pub source_label: String,
    pub version_label: String,
}

#[derive(Debug)]
pub struct InstallSuccessView {
    pub site_name: String,
    pub command_count: usize,
    pub hint: String,
}

pub async fn run_cli(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Recon(args) => recon::run(args).await,
        Commands::Emit(args) => emit::run(args).await,
        Commands::Install(args) => install::run(args).await,
        Commands::Lint(args) => lint::run(args),
        Commands::Exec(args) => exec_command(args).await,
        Commands::Auth(args) => auth_command(args).await,
        Commands::Check(args) => check_command(args).await,
        Commands::Shell => shell_command().await,
    }
}

pub async fn exec_command(args: ExecArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let ir_path = webctl_ir::site_ir_path(&home, &args.site);

    if !ir_path.exists() {
        let registry_path = webctl_ir::registry_path(&home);
        let registry = webctl_ir::RegistryIndex::load(&registry_path).unwrap_or_else(|_| webctl_ir::RegistryIndex { sites: Vec::new() });

        if let Some(entry) = registry.find(&args.site) {
            let descriptor = webctl_ir::read_ir(&entry.ir_path)
                .with_context(|| format!("failed to read IR for site '{}'", args.site))?;
            return exec_with_ir(&descriptor, &args).await;
        }

        let installed = registry.sites.iter().map(|s| &s.site_name).collect::<Vec<_>>();
        if installed.is_empty() {
            return Err(anyhow!("site '{}' not found. No sites installed.\n\n  Install one with:\n    webctl recon <url> --auto --yes\n    webctl install <ir_path>", args.site));
        }
        return Err(anyhow!("site '{}' not found. Installed sites:\n{}\n\n  Install with:\n    webctl install <ir_path>",
            args.site,
            installed.iter().map(|s| format!("    {s}")).collect::<Vec<_>>().join("\n")
        ));
    }

    let descriptor = webctl_ir::read_ir(&ir_path)
        .with_context(|| format!("failed to read IR for site '{}'", args.site))?;
    exec_with_ir(&descriptor, &args).await
}

async fn exec_with_ir(descriptor: &webctl_ir::SiteDescriptor, args: &ExecArgs) -> anyhow::Result<()> {
    let is_help = args.args.is_empty()
        || args.args.iter().any(|a| a == "--help" || a == "-h");

    let is_open = args.args.first().map(|a| a.as_str()) == Some("open");
    if is_open {
        let remaining: Vec<&str> = args.args[1..].iter().map(|s| s.as_str()).collect();

        let (from_cmd, index_str) = if remaining.len() >= 2 {
            if let Ok(idx) = remaining.last().unwrap().parse::<usize>() {
                (Some(remaining[..remaining.len()-1].join(" ")), idx)
            } else {
                return Err(anyhow!("usage: {site} open [command] <index>\n\nExamples:\n  {site} open 1           Open item #1 from last listed command\n  {site} open news 3      Open item #3 from 'news' command", site = args.site));
            }
        } else if remaining.len() == 1 {
            let idx: usize = remaining[0].parse().context(
                format!("usage: {site} open [command] <index>\n\nExamples:\n  {site} open 1           Open item #1\n  {site} open news 3      Open item #3 from 'news'", site = args.site)
            )?;
            (None, idx)
        } else {
            return Err(anyhow!("usage: {site} open [command] <index>", site = args.site));
        };

        return open_item_by_index(descriptor, &args.site, index_str, from_cmd.as_deref()).await;
    }

    if is_help {
        let help = if crate::execute::use_color() {
            webctl_emit_cli::build_help_text_colored(descriptor)
        } else {
            webctl_emit_cli::build_help_text(descriptor)
        };
        println!("{help}");
        return Ok(());
    }

    let is_json = args.args.iter().any(|a| a == "--json");
    let cmd_args: Vec<&str> = args.args.iter()
        .map(|s| s.as_str())
        .filter(|s| *s != "--json")
        .collect();

    let command_key = cmd_args.join(" ");
    let operation = descriptor.operations.iter().find(|op| {
        op.command_path.join(" ") == command_key
    });

    let Some(operation) = operation else {
        let available = webctl_ir::command_help_rows(descriptor)
            .iter()
            .map(|r| format!("    {}  {}", r.command, r.description))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(anyhow!(
            "unknown command '{}' for site '{}'\n\nAvailable commands:\n{}\n",
            command_key, args.site, available
        ));
    };

    let url = match &operation.transport {
        webctl_ir::OperationTransport::Http(http_op) => {
            let endpoint = descriptor.http.as_ref()
                .and_then(|h| h.endpoints.get(http_op.endpoint_index))
                .ok_or_else(|| anyhow!("endpoint index {} out of bounds", http_op.endpoint_index))?;
            let base = url::Url::parse(&descriptor.meta.source_url)
                .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("localhost")))
                .unwrap_or_else(|_| descriptor.meta.source_url.clone());
            format!("{}{}", base, endpoint.path)
        }
        webctl_ir::OperationTransport::Ax(_) => {
            return Err(anyhow!("AX-based command execution is not yet implemented"));
        }
    };

    if let Some(ref extractor) = operation.extractor {
        save_last_command(&args.site, &command_key);

        let session = crate::execute::get_session_name(&args.site);
        let html = if let Some(ref session_name) = session {
            crate::execute::fetch_authenticated_html(&url, session_name).await
                .with_context(|| format!("failed to fetch {url} with authenticated session"))?
        } else {
            crate::execute::fetch_raw_html(&url).await
                .with_context(|| format!("failed to fetch {url}"))?
        };

        if let Some(items) = crate::extract::extract_items(&html, extractor) {
            if is_json {
                let json = crate::execute::format_extracted_json(&items, &url, &descriptor.meta.display_name)?;
                println!("{json}");
            } else {
                let formatted = crate::execute::format_extracted_human(
                    &items, &args.site, &command_key, &descriptor.meta.display_name, descriptor,
                );
                println!("{formatted}");
            }
            return Ok(());
        }
    }

    let session = crate::execute::get_session_name(&args.site);
    let result = if let Some(ref session_name) = session {
        let html = crate::execute::fetch_authenticated_html(&url, session_name).await
            .with_context(|| format!("failed to fetch {url} with authenticated session"))?;
        let text = crate::execute::html_to_text_public(&html);
        crate::execute::ExecResult {
            status: 200,
            url: url.clone(),
            title: descriptor.meta.display_name.clone(),
            content: text,
            word_count: 0,
        }
    } else {
        crate::execute::fetch_page(&url).await
            .with_context(|| format!("failed to fetch {url}"))?
    };

    if is_json {
        let json = crate::execute::format_json(&result)?;
        println!("{json}");
    } else {
        let formatted = crate::execute::format_human(&result, &args.site, &command_key, descriptor);
        println!("{formatted}");
    }

    Ok(())
}

pub async fn recon_command(args: ReconArgs) -> anyhow::Result<webctl_ir::SiteDescriptor> {
    let output_dir = recon_output_dir(&args.url, args.output.as_deref())?;
    std::fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create output dir {}", output_dir.display()))?;

    let probe_options = webctl_probe::ProbeOptions {
        url: args.url.clone(),
        visible: true,
        output_dir: output_dir.clone(),
    };

    step("Connecting to browser on port 9222...");
    let browser = webctl_probe::agent_browser::BrowserProcess {
        child_id: 0,
        cdp_port: 9222,
        profile_dir: std::path::PathBuf::new(),
    };
    let session = match webctl_probe::agent_browser::connect_session(browser, &probe_options).await {
        Ok(s) => s,
        Err(e) => {
            err("Cannot connect to browser on port 9222");
            eprintln!();
            hint("webctl needs a Chromium browser with remote debugging enabled.");
            hint("Start one with:");
            eprintln!();
            hint("# Comet (Perplexity)");
            cmd("/Applications/Comet.app/Contents/MacOS/Comet --remote-debugging-port=9222 &");
            eprintln!();
            hint("# Chrome");
            cmd("/Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome --remote-debugging-port=9222 &");
            eprintln!();
            hint("Then retry:");
            cmd(&format!("webctl recon {} {}", args.url, if args.auto { "--auto" } else { "" }));
            return Err(e);
        }
    };
    ok("Connected");

    step("Starting HAR capture...");
    webctl_probe::agent_browser::start_har_capture(&session).await?;

    let initial_title = webctl_probe::agent_browser::get_title(&session).await.ok();
    let initial_url = webctl_probe::agent_browser::get_url(&session).await.ok();
    ok(&format!("Navigated to: {}", initial_title.as_deref().unwrap_or(&args.url)));

    let ax_pre_path = webctl_probe::paths::ax_pre_path(&output_dir);
    webctl_probe::agent_browser::take_ax_snapshot(&session, &ax_pre_path).await?;

    if args.auto {
        step(&format!("Auto-exploring {}...", args.url));
        let color = use_color();
        let auto_result = webctl_probe::run_auto_recon(&session, move |iter, elements, url| {
            if color {
                eprint!("\r  {} {}/15  {} {:<4}  {} {}          ",
                    "iter".dimmed(), iter.to_string().bold(),
                    "elements:".dimmed(), elements,
                    "→".dimmed(), url.cyan()
                );
            } else {
                eprint!("\r  iter {iter}/15  elements: {elements:<4}  current: {url}          ");
            }
        })
        .await
        .context("auto-recon failed")?;
        eprintln!();
        ok(&format!("Explored {} pages in {} iterations",
            auto_result.pages_visited, auto_result.iterations
        ));
        hint(&auto_result.stop_reason);
    } else {
        step("Navigate the site in the browser window, then press ENTER here when done.");
        wait_for_enter().await?;
    }

    step("Finalizing capture...");
    let mut probe = webctl_probe::finalize_capture(session).await?;

    if let Some(ref title) = initial_title {
        if !title.is_empty() {
            probe.final_title = Some(title.clone());
        }
    }
    if let Some(ref url) = initial_url {
        if !url.is_empty() && probe.final_url.is_none() {
            probe.final_url = Some(url.clone());
        }
    }

    ok(&format!("Captured {} HTTP requests", probe.har_entry_count));
    step("Classifying site...");
    let har_bytes = webctl_probe::read_har_bytes(&probe.har_path)?;
    let har = webctl_probe::har::parse_har(&har_bytes)?;
    let ax_tree = read_optional_string(probe.ax_final_path.as_deref())?;
    let suggestion = webctl_classifier::classify(&probe, &har_bytes, ax_tree.as_deref())?;

    let http_surface = webctl_ir::HttpSurface {
        endpoints: infer_endpoints(&har),
    };
    let ax_surface = ax_tree
        .as_deref()
        .map(|text| webctl_ir::AxSurface {
            actions: extract_ax_actions(text),
        });

    let view = ClassifierSuggestionView {
        bucket: suggestion.bucket.clone(),
        confidence: suggestion.confidence.clone(),
        http_endpoint_count: http_surface.endpoints.len(),
        ax_action_count: ax_surface.as_ref().map(|surface| surface.actions.len()).unwrap_or(0),
    };
    ok(&format!("Classified: {} ({} confidence)",
        classifier_bucket_label(&view.bucket),
        confidence_label(&view.confidence),
    ));
    hint(&format!("HTTP endpoints: {}  |  AX actions: {}", view.http_endpoint_count, view.ax_action_count));

    let decision = apply_user_override(suggestion, args.technique_override());

    if !args.yes {
        let technique = technique_label(&decision);
        let accepted = prompt::confirm(&format!("Build IR using {technique}?"))?;
        if !accepted {
            return Err(anyhow!(abort_recon().to_string()));
        }
    }

    step("Detecting extractors...");
    let mut endpoint_extractors: std::collections::HashMap<usize, webctl_ir::Extractor> =
        std::collections::HashMap::new();
    let will_use_http = matches!(
        decision,
        webctl_classifier::TechniqueDecision::HttpOnly | webctl_classifier::TechniqueDecision::Hybrid
    );
    if will_use_http {
        for (i, endpoint) in http_surface.endpoints.iter().enumerate() {
            let ep_url = {
                let base = initial_url.as_deref().and_then(|u| {
                    url::Url::parse(u).ok().map(|p| format!("{}://{}", p.scheme(), p.host_str().unwrap_or("localhost")))
                }).unwrap_or_else(|| args.url.clone());
                format!("{}{}", base.trim_end_matches('/'), endpoint.path)
            };
            match crate::execute::fetch_raw_html(&ep_url).await {
                Ok(html) => {
                    if let Some(extractor) = webctl_probe::auto_extract::detect_extractor(&html, &ep_url).await {
                        let field_count = match &extractor {
                            webctl_ir::Extractor::List(l) => l.fields.len(),
                            _ => 0,
                        };
                        hint(&format!("  {} → {} fields detected", endpoint.path, field_count));
                        endpoint_extractors.insert(i, extractor);
                    }
                }
                Err(_) => {}
            }
        }
    }
    let extractor_count = endpoint_extractors.len();
    ok(&format!("Detected extractors for {} endpoints", extractor_count));

    step("Building IR...");
    let descriptor = build_ir(decision, probe, http_surface, ax_surface, endpoint_extractors)?;
    let ir_path = output_dir.join(format!("{}.webctl.json", descriptor.meta.site_name));
    webctl_ir::write_ir(&ir_path, &descriptor)
        .with_context(|| format!("failed to write IR to {}", ir_path.display()))?;

    let op_count = descriptor.operations.len();
    let ir_size = std::fs::metadata(&ir_path).map(|m| m.len()).unwrap_or(0);
    ok(&format!("IR written: {} ({} operations, {}KB)", ir_path.display(), op_count, ir_size / 1024));
    eprintln!();
    hint("Next steps:");
    cmd(&format!("webctl install {}     Install locally", ir_path.display()));
    cmd(&format!("webctl lint {}        Validate the IR", ir_path.display()));
    cmd(&format!("webctl emit cli {}    Generate a CLI shim", ir_path.display()));

    Ok(descriptor)
}

pub fn apply_user_override(
    suggestion: webctl_classifier::ClassificationResult,
    override_choice: Option<TechniqueOverride>,
) -> webctl_classifier::TechniqueDecision {
    match override_choice {
        Some(TechniqueOverride::Http) => webctl_classifier::TechniqueDecision::HttpOnly,
        Some(TechniqueOverride::Ax) => webctl_classifier::TechniqueDecision::AxOnly,
        Some(TechniqueOverride::Hybrid) => webctl_classifier::TechniqueDecision::Hybrid,
        None => match suggestion.bucket {
            webctl_classifier::ClassifierBucket::FormSessionLegacy
            | webctl_classifier::ClassifierBucket::RestModernSpa
            | webctl_classifier::ClassifierBucket::GraphqlIntrospectable
            | webctl_classifier::ClassifierBucket::HtmlRendered => {
                webctl_classifier::TechniqueDecision::HttpOnly
            }
            webctl_classifier::ClassifierBucket::AxOnly => {
                webctl_classifier::TechniqueDecision::AxOnly
            }
            webctl_classifier::ClassifierBucket::Hostile
            | webctl_classifier::ClassifierBucket::Inconclusive => {
                if suggestion.features.ax_interactive_nodes > 0
                    && suggestion.features.total_requests > 0
                {
                    webctl_classifier::TechniqueDecision::Hybrid
                } else if suggestion.features.ax_interactive_nodes > 0 {
                    webctl_classifier::TechniqueDecision::AxOnly
                } else {
                    webctl_classifier::TechniqueDecision::HttpOnly
                }
            }
        },
    }
}

pub fn abort_recon() -> miette::Report {
    miette::miette!("recon aborted")
}

pub fn build_ir(
    decision: webctl_classifier::TechniqueDecision,
    probe: webctl_probe::ProbeCapture,
    http: webctl_ir::HttpSurface,
    ax: Option<webctl_ir::AxSurface>,
    endpoint_extractors: std::collections::HashMap<usize, webctl_ir::Extractor>,
) -> anyhow::Result<webctl_ir::SiteDescriptor> {
    let source_url = probe
        .final_url
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let site_name = site_slug_from_url(&source_url).unwrap_or_else(|| "site".to_string());
    let display_name = probe
        .final_title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| site_name.clone());
    let technique = match decision {
        webctl_classifier::TechniqueDecision::HttpOnly => webctl_ir::ProvenanceTechnique::Http,
        webctl_classifier::TechniqueDecision::AxOnly => webctl_ir::ProvenanceTechnique::Ax,
        webctl_classifier::TechniqueDecision::Hybrid => webctl_ir::ProvenanceTechnique::Hybrid,
        webctl_classifier::TechniqueDecision::Abort => return Err(anyhow!("recon aborted")),
    };

    let mut operations = Vec::new();
    let http_enabled = matches!(
        decision,
        webctl_classifier::TechniqueDecision::HttpOnly
            | webctl_classifier::TechniqueDecision::Hybrid
    );
    let ax_enabled = matches!(
        decision,
        webctl_classifier::TechniqueDecision::AxOnly | webctl_classifier::TechniqueDecision::Hybrid
    );

    if http_enabled {
        for (index, endpoint) in http.endpoints.iter().enumerate() {
            let command_path = if endpoint.namespace.is_empty() {
                fallback_http_command_path(index, &endpoint.path)
            } else {
                webctl_ir::normalize_command_path(&endpoint.namespace)
            };
            let summary = if endpoint.description.trim().is_empty() {
                format!("{:?} {}", endpoint.method, endpoint.path)
            } else {
                endpoint.description.clone()
            };
            let description = if endpoint.description.trim().is_empty() {
                summary.clone()
            } else {
                endpoint.description.clone()
            };
            let extractor = endpoint_extractors.get(&index).cloned();
            operations.push(webctl_ir::OperationDescriptor {
                command_path,
                summary,
                description,
                operation_kind: endpoint.operation_kind.clone(),
                transport: webctl_ir::OperationTransport::Http(webctl_ir::HttpOperation {
                    endpoint_index: index,
                }),
                extractor,
            });
        }
    }

    if ax_enabled {
        if let Some(ref ax_surface) = ax {
            for (index, action) in ax_surface.actions.iter().enumerate() {
                let command_path = if action.command_path.is_empty() {
                    vec![format!("action-{}", index + 1)]
                } else {
                    action.command_path.clone()
                };
                let description = if action.description.trim().is_empty() {
                    format!("AX action {}", index + 1)
                } else {
                    action.description.clone()
                };
                operations.push(webctl_ir::OperationDescriptor {
                    command_path,
                    summary: description.clone(),
                    description,
                    operation_kind: webctl_ir::OperationKind::Other,
                    transport: webctl_ir::OperationTransport::Ax(webctl_ir::AxOperation {
                        action_index: index,
                    }),
                    extractor: None,
                });
            }
        }
    }

    if operations.is_empty() {
        return Err(anyhow!("no operations were derived from the selected technique"));
    }

    Ok(webctl_ir::SiteDescriptor {
        meta: webctl_ir::SiteMeta {
            site_name,
            display_name,
            source_url,
            ir_version: "0.1.0".to_string(),
        },
        provenance: webctl_ir::Provenance {
            generated_at: timestamp_string()?,
            technique,
            classifier_bucket: technique_bucket_label(&decision).to_string(),
            probe_duration_sec: 0,
        },
        operations,
        http: http_enabled.then_some(http),
        ax: ax_enabled.then_some(ax).flatten(),
    })
}

pub async fn emit_command(args: EmitArgs) -> anyhow::Result<std::path::PathBuf> {
    let (ir_path, target_label) = match &args.target {
        EmitTargetArg::Cli { ir_path } => (ir_path.clone(), "cli"),
        EmitTargetArg::JustBash { ir_path } => (ir_path.clone(), "just-bash"),
    };
    let descriptor = webctl_ir::read_ir(&ir_path)
        .with_context(|| format!("failed to read IR from {}", ir_path.display()))?;

    if let Err(errors) = webctl_ir::lint_ir(&descriptor) {
        for error in errors {
            eprintln!("warning: {error}");
        }
    }

    if target_label == "just-bash" {
        let out_dir = args.out_dir.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("emit")
                .join("just-bash")
        });
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("failed to create {}", out_dir.display()))?;

        let config_content = webctl_emit_cli::emit_executor_config(&descriptor);
        let config_path = out_dir.join(format!("{}.config.js", descriptor.meta.site_name));
        std::fs::write(&config_path, &config_content)
            .with_context(|| format!("failed to write {}", config_path.display()))?;

        let size = config_content.len();
        ok(&format!("ExecutorConfig written: {} ({}B)", config_path.display(), size));
        eprintln!();
        hint("Usage in just-bash:");
        cmd("import { Bash } from \"just-bash\";");
        cmd("import { createExecutor } from \"@just-bash/executor\";");
        cmd(&format!(
            "import {{ createWebctlExecutor }} from \"./{}.config.js\";",
            descriptor.meta.site_name
        ));
        cmd("const executor = await createWebctlExecutor(createExecutor);");
        cmd("const bash = new Bash({ javascript: { invokeTool: executor.invokeTool }, customCommands: executor.commands });");
        cmd(&format!("await bash.exec('{} --help');", descriptor.meta.site_name));

        return Ok(config_path);
    }

    let out_dir = args.out_dir.unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("emit")
            .join("cli")
            .join(&descriptor.meta.site_name)
    });
    let emitted = webctl_emit_cli::emit_cli_shim(webctl_emit_cli::CliEmitRequest {
        descriptor,
        out_dir,
    })?;

    ok(&format!("Shim compiled: {} ({}KB)",
        emitted.binary_path.display(),
        emitted.binary_size / 1024
    ));
    eprintln!();
    hint("Next step:");
    cmd(&format!("webctl install {}", ir_path.display()));

    Ok(emitted.binary_path)
}

pub async fn install_command(args: InstallArgs) -> anyhow::Result<InstallSuccessView> {
    let source_path = resolve_local_ir_source(&args.source)?;
    let source_label = source_path.display().to_string();

    let descriptor = webctl_ir::read_ir(&source_path)
        .with_context(|| format!("failed to read IR from {}", source_path.display()))?;
    let progress = InstallProgressView {
        source_label,
        version_label: descriptor.meta.ir_version.clone(),
    };
    render_install_progress(&progress);

    if let Err(errors) = webctl_ir::lint_ir(&descriptor) {
        for error in errors {
            println!("warning: {error}");
        }
    }

    let home_dir = home_dir()?;
    let site_ir_path = webctl_ir::site_ir_path(&home_dir, &descriptor.meta.site_name);
    webctl_ir::write_ir(&site_ir_path, &descriptor)
        .with_context(|| format!("failed to copy IR to {}", site_ir_path.display()))?;

    let shim_build_dir = std::env::temp_dir().join(format!(
        "webctl-install-{}-{}",
        descriptor.meta.site_name,
        std::process::id()
    ));
    let emitted = webctl_emit_cli::emit_cli_shim(webctl_emit_cli::CliEmitRequest {
        descriptor: descriptor.clone(),
        out_dir: shim_build_dir,
    })?;

    let dest_dir = install_destination(&args)?;
    std::fs::create_dir_all(&dest_dir)
        .with_context(|| format!("failed to create destination dir {}", dest_dir.display()))?;
    let installed_shim_path = dest_dir.join(&descriptor.meta.site_name);
    std::fs::copy(&emitted.binary_path, &installed_shim_path).with_context(|| {
        format!(
            "failed to copy shim from {} to {}",
            emitted.binary_path.display(),
            installed_shim_path.display()
        )
    })?;

    let registry_path = webctl_ir::registry_path(&home_dir);
    let mut registry = webctl_ir::RegistryIndex::load(&registry_path)
        .with_context(|| format!("failed to load registry at {}", registry_path.display()))?;
    registry.upsert(webctl_ir::InstalledSiteEntry {
        site_name: descriptor.meta.site_name.clone(),
        ir_path: site_ir_path.clone(),
        shim_path: installed_shim_path.clone(),
    });
    registry
        .save(&registry_path)
        .with_context(|| format!("failed to write registry at {}", registry_path.display()))?;

    let meta_path = webctl_ir::site_meta_path(&home_dir, &descriptor.meta.site_name);
    if let Some(parent) = meta_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create site metadata dir {}", parent.display()))?;
    }
    let install_record = webctl_ir::InstallRecord {
        site_name: descriptor.meta.site_name.clone(),
        ir_path: site_ir_path,
        shim_path: installed_shim_path,
        installed_at: timestamp_string()?,
        source: webctl_ir::InstallSource::LocalPath(webctl_ir::LocalPathSource {
            path: source_path,
        }),
    };
    std::fs::write(&meta_path, serde_json::to_vec_pretty(&install_record)?)
        .with_context(|| format!("failed to write install metadata to {}", meta_path.display()))?;

    let in_path = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .any(|p| std::path::Path::new(p) == dest_dir);

    let hint_text = if in_path {
        format!("Run: {} --help", descriptor.meta.site_name)
    } else {
        format!(
            "⚠ {} is not in your PATH. Add it:\n  echo 'export PATH=\"{}:$PATH\"' >> ~/.zshrc && source ~/.zshrc\n\nThen try:\n  {} --help",
            dest_dir.display(),
            dest_dir.display(),
            descriptor.meta.site_name
        )
    };

    let view = InstallSuccessView {
        site_name: descriptor.meta.site_name.clone(),
        command_count: descriptor.operations.len(),
        hint: hint_text,
    };
    ok(&format!("Installed: {} ({} commands)", view.site_name, view.command_count));
    eprintln!("{}", view.hint);

    Ok(view)
}

pub fn render_install_progress(view: &InstallProgressView) {
    if use_color() {
        eprintln!("{} {} {} ({})",
            "⠋".cyan(),
            "Installing from".dimmed(),
            view.source_label.white(),
            format!("v{}", view.version_label).dimmed()
        );
    } else {
        eprintln!("⠋ Installing IR from {} (version {})", view.source_label, view.version_label);
    }
}

async fn open_item_by_index(
    descriptor: &webctl_ir::SiteDescriptor,
    site_name: &str,
    index: usize,
    from_command: Option<&str>,
) -> anyhow::Result<()> {
    let operation = if let Some(cmd) = from_command {
        descriptor.operations.iter().find(|op| {
            op.command_path.join(" ") == cmd && op.extractor.is_some()
        }).or_else(|| {
            descriptor.operations.iter().find(|op| op.extractor.is_some())
        })
    } else {
        load_last_command(site_name)
            .and_then(|last| {
                descriptor.operations.iter().find(|op| {
                    op.command_path.join(" ") == last && op.extractor.is_some()
                })
            })
            .or_else(|| {
                descriptor.operations.iter().find(|op| op.extractor.is_some())
            })
    };

    let Some(operation) = operation else {
        return Err(anyhow!("no extractors configured for site '{site_name}'. Re-run recon to detect structure."));
    };

    let extractor = operation.extractor.as_ref().unwrap();
    let url = match &operation.transport {
        webctl_ir::OperationTransport::Http(http_op) => {
            let endpoint = descriptor.http.as_ref()
                .and_then(|h| h.endpoints.get(http_op.endpoint_index))
                .context("endpoint not found")?;
            let base = url::Url::parse(&descriptor.meta.source_url)
                .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("localhost")))
                .unwrap_or_else(|_| descriptor.meta.source_url.clone());
            format!("{}{}", base, endpoint.path)
        }
        _ => return Err(anyhow!("open requires an HTTP operation")),
    };

    step(&format!("Fetching {} to find item #{}...", operation.command_path.join(" "), index));
    let html = crate::execute::fetch_raw_html(&url).await?;

    let items = crate::extract::extract_items(&html, extractor)
        .context("extraction failed — no items found on page")?;

    let item = items.iter().find(|i| i.index == index)
        .ok_or_else(|| anyhow!("item #{index} not found. Page has {} items (1-{})", items.len(), items.len()))?;

    let item_url = item.primary_url()
        .ok_or_else(|| anyhow!("item #{index} has no URL field"))?;

    let title = item.primary_title().unwrap_or("(untitled)");
    ok(&format!("Opening: {}", title));
    hint(item_url);

    open_url_in_browser(item_url).await?;
    Ok(())
}

async fn shell_command() -> anyhow::Result<()> {
    crate::shell::run_shell().await
}

async fn auth_command(args: AuthArgs) -> anyhow::Result<()> {
    match args.action {
        AuthAction::Login(login) => auth_login(login).await,
        AuthAction::Status(status) => auth_status(status).await,
        AuthAction::Logout(logout) => auth_logout(logout).await,
    }
}

async fn auth_login(args: AuthLoginArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let ir_path = webctl_ir::site_ir_path(&home, &args.site);
    if !ir_path.exists() {
        return Err(anyhow!("site '{}' not installed. Run: webctl install <ir_path>", args.site));
    }

    let descriptor = webctl_ir::read_ir(&ir_path)?;
    let login_url = args.url.clone().unwrap_or_else(|| descriptor.meta.source_url.clone());
    let session_name = format!("webctl-{}", args.site);

    step(&format!("Opening {} for authentication...", login_url));
    hint("Log in normally in the browser window.");
    hint("Press ENTER here when you're logged in.");

    let browser = webctl_probe::agent_browser::BrowserProcess {
        child_id: 0,
        cdp_port: 9222,
        profile_dir: std::path::PathBuf::new(),
    };
    let probe_opts = webctl_probe::ProbeOptions {
        url: login_url.clone(),
        visible: true,
        output_dir: std::env::temp_dir().join(format!("webctl-auth-{}", args.site)),
    };
    let session = webctl_probe::agent_browser::connect_session(browser, &probe_opts).await
        .context("failed to connect to browser. Is Comet running with --remote-debugging-port=9222?")?;

    ok(&format!("Browser opened: {}", login_url));
    eprintln!();
    step("Waiting for you to log in... press ENTER when done.");
    wait_for_enter().await?;

    let session_dir = webctl_ir::site_dir(&home, &args.site);
    std::fs::create_dir_all(&session_dir)?;
    let session_file = session_dir.join("session-name");
    std::fs::write(&session_file, &session_name)?;

    ok(&format!("Session saved for '{}'. Commands will now use authenticated state.", args.site));
    hint(&format!("Try: {} --help", args.site));

    Ok(())
}

async fn auth_status(args: AuthStatusArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let session_file = webctl_ir::site_dir(&home, &args.site).join("session-name");

    if session_file.exists() {
        let session = std::fs::read_to_string(&session_file)?;
        ok(&format!("Site '{}' has authenticated session: {}", args.site, session));
    } else {
        warn(&format!("Site '{}' has no authenticated session.", args.site));
        hint(&format!("Run: webctl auth login {}", args.site));
    }
    Ok(())
}

async fn auth_logout(args: AuthLogoutArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let session_file = webctl_ir::site_dir(&home, &args.site).join("session-name");

    if session_file.exists() {
        std::fs::remove_file(&session_file)?;
        ok(&format!("Logged out of '{}'.", args.site));
    } else {
        hint(&format!("Site '{}' was not logged in.", args.site));
    }
    Ok(())
}

pub async fn check_command(args: CheckArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let ir_path = webctl_ir::site_ir_path(&home, &args.site);
    if !ir_path.exists() {
        return Err(anyhow!("site '{}' not installed", args.site));
    }

    let descriptor = webctl_ir::read_ir(&ir_path)?;
    let fingerprint_path = webctl_ir::site_dir(&home, &args.site).join("fingerprint.json");

    let base = url::Url::parse(&descriptor.meta.source_url)
        .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("localhost")))
        .unwrap_or_else(|_| descriptor.meta.source_url.clone());

    let canaries: Vec<&webctl_ir::HttpEndpoint> = descriptor
        .http
        .as_ref()
        .map(|h| h.endpoints.iter().take(3).collect())
        .unwrap_or_default();

    if canaries.is_empty() {
        warn("No HTTP endpoints to check drift against.");
        return Ok(());
    }

    step(&format!("Checking {} canary endpoints for {}...", canaries.len(), args.site));

    let mut current: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for ep in &canaries {
        let url = format!("{}{}", base, ep.path);
        match reqwest_head(&url).await {
            Ok(sig) => {
                current.insert(ep.path.clone(), sig);
            }
            Err(e) => {
                warn(&format!("  {} → failed: {}", ep.path, e));
            }
        }
    }

    if current.is_empty() {
        err("Could not reach any canary endpoints.");
        return Err(anyhow!("drift check failed — site unreachable"));
    }

    if !fingerprint_path.exists() {
        let json = serde_json::to_string_pretty(&current)?;
        std::fs::write(&fingerprint_path, &json)?;
        ok(&format!("Baseline fingerprint saved ({} endpoints)", current.len()));
        hint("Run 'webctl check' again later to detect drift.");
        return Ok(());
    }

    let stored: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&std::fs::read_to_string(&fingerprint_path)?)?;

    let mut drifted = Vec::new();
    let mut matched = 0;

    for (path, current_sig) in &current {
        if let Some(stored_sig) = stored.get(path) {
            if current_sig != stored_sig {
                drifted.push(path.clone());
            } else {
                matched += 1;
            }
        } else {
            drifted.push(path.clone());
        }
    }

    if drifted.is_empty() {
        ok(&format!("No drift detected ({}/{} endpoints match)", matched, current.len()));
    } else {
        warn(&format!("Drift detected in {} endpoint(s):", drifted.len()));
        for path in &drifted {
            hint(&format!("  {path}"));
        }
        eprintln!();
        hint("The site may have changed. Re-run recon to update:");
        cmd(&format!("webctl recon {} --auto --yes", descriptor.meta.source_url));
        cmd(&format!("webctl install <new-ir-path>"));

        let json = serde_json::to_string_pretty(&current)?;
        std::fs::write(&fingerprint_path, &json)?;
        hint("Fingerprint updated.");
    }

    Ok(())
}

async fn reqwest_head(url: &str) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("curl")
        .args(["-sf", "-o", "/dev/null", "-w", "%{http_code}|%{content_type}|%{size_download}", url])
        .output()
        .await
        .context("failed to curl")?;

    if !output.status.success() {
        return Err(anyhow!("curl failed"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn save_last_command(site_name: &str, command: &str) {
    if let Ok(home) = std::env::var("HOME") {
        let state_path = webctl_ir::site_dir(&std::path::PathBuf::from(home), site_name)
            .join("last-command");
        let _ = std::fs::write(state_path, command);
    }
}

fn load_last_command(site_name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let state_path = webctl_ir::site_dir(&std::path::PathBuf::from(home), site_name)
        .join("last-command");
    std::fs::read_to_string(state_path).ok()
}

async fn open_url_in_browser(url: &str) -> anyhow::Result<()> {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    };

    tokio::process::Command::new(cmd)
        .arg(url)
        .spawn()
        .with_context(|| format!("failed to open browser with {cmd}"))?;

    Ok(())
}

fn fallback_http_command_path(index: usize, endpoint_path: &str) -> Vec<String> {
    let segments = endpoint_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .take(2)
        .map(|segment| {
            segment
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '-' {
                        ch.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect::<String>()
                .trim_matches('-')
                .to_string()
        })
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        vec![format!("endpoint-{}", index + 1)]
    } else {
        segments
    }
}

fn technique_label(decision: &webctl_classifier::TechniqueDecision) -> &'static str {
    match decision {
        webctl_classifier::TechniqueDecision::HttpOnly => "http",
        webctl_classifier::TechniqueDecision::AxOnly => "ax",
        webctl_classifier::TechniqueDecision::Hybrid => "hybrid",
        webctl_classifier::TechniqueDecision::Abort => "abort",
    }
}

fn classifier_bucket_label(bucket: &webctl_classifier::ClassifierBucket) -> &'static str {
    match bucket {
        webctl_classifier::ClassifierBucket::FormSessionLegacy => "FormSessionLegacy",
        webctl_classifier::ClassifierBucket::RestModernSpa => "RestModernSpa",
        webctl_classifier::ClassifierBucket::GraphqlIntrospectable => "GraphqlIntrospectable",
        webctl_classifier::ClassifierBucket::AxOnly => "AxOnly",
        webctl_classifier::ClassifierBucket::HtmlRendered => "HtmlRendered",
        webctl_classifier::ClassifierBucket::Hostile => "Hostile",
        webctl_classifier::ClassifierBucket::Inconclusive => "Inconclusive",
    }
}

fn confidence_label(confidence: &webctl_classifier::Confidence) -> &'static str {
    match confidence {
        webctl_classifier::Confidence::High => "high",
        webctl_classifier::Confidence::Medium => "medium",
        webctl_classifier::Confidence::Low => "low",
    }
}

fn technique_bucket_label(decision: &webctl_classifier::TechniqueDecision) -> &'static str {
    match decision {
        webctl_classifier::TechniqueDecision::HttpOnly => "HttpOnly",
        webctl_classifier::TechniqueDecision::AxOnly => "AxOnly",
        webctl_classifier::TechniqueDecision::Hybrid => "Hybrid",
        webctl_classifier::TechniqueDecision::Abort => "Abort",
    }
}

fn home_dir() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

fn install_destination(args: &InstallArgs) -> anyhow::Result<PathBuf> {
    match &args.dest {
        Some(path) => Ok(path.clone()),
        None => Ok(home_dir()?.join(".local").join("bin")),
    }
}

fn resolve_local_ir_source(source: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(source);
    if path.exists() {
        return Ok(path);
    }
    Err(anyhow!(
        "unsupported IR source `{source}`; only local file paths are supported in this build"
    ))
}

fn recon_output_dir(url: &str, output: Option<&Path>) -> anyhow::Result<PathBuf> {
    if let Some(output) = output {
        return Ok(output.to_path_buf());
    }

    let slug = site_slug_from_url(url).unwrap_or_else(|| sanitize_slug(url));
    Ok(std::env::current_dir()
        .context("failed to determine current directory")?
        .join(format!("webctl-recon-{slug}")))
}

fn site_slug_from_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    Some(sanitize_slug(host))
}

fn sanitize_slug(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut last_dash = false;

    for ch in input.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '-'
        };

        if next == '-' {
            if !last_dash {
                slug.push('-');
            }
            last_dash = true;
        } else {
            slug.push(next);
            last_dash = false;
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "site".to_string()
    } else {
        slug
    }
}

async fn wait_for_enter() -> anyhow::Result<()> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .context("failed to read ENTER confirmation from stdin")?;
    Ok(())
}

fn read_optional_string(path: Option<&Path>) -> anyhow::Result<Option<String>> {
    path.map(|path| {
        std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))
    })
    .transpose()
}

fn timestamp_string() -> anyhow::Result<String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_secs();
    Ok(format!("{now}"))
}

pub fn flush_stdout() -> anyhow::Result<()> {
    io::stdout().flush().context("failed to flush stdout")
}
