use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand};
use owo_colors::OwoColorize;
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::commands::{emit, install, lint};
use crate::output::emit_json;

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
    name = "surfacer",
    about = "Generate the interface instead of writing it",
    after_help = "Getting started:\n  surfacer lint <ir-path>       Validate an IR\n  surfacer install <ir-path>    Install it as a command\n\nLearn more:\n  https://github.com/crafter-station/surfacer"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Emit(EmitArgs),
    Install(InstallArgs),
    Lint(LintArgs),
    Exec(ExecArgs),
    Auth(AuthArgs),
    Check(CheckArgs),
    Shell,
    Skills(SkillsArgs),
    /// Print this CLI's own command surface as JSON.
    Schema,
}

/// Serves the agent-facing manual that ships inside the binary.
///
/// The manual is embedded rather than published as a separate file so it
/// cannot drift from the version installed. A copy kept in a vault or a
/// skills directory goes stale silently; this one is whatever the running
/// binary was built from.
#[derive(Debug, Clone, clap::Args)]
pub struct SkillsArgs {
    #[command(subcommand)]
    pub action: SkillsAction,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SkillsAction {
    /// List the available skill documents.
    List,
    /// Print one skill document to stdout.
    Get(SkillsGetArgs),
}

#[derive(Debug, Clone, clap::Args)]
pub struct SkillsGetArgs {
    /// Which document to print. Defaults to the only one that exists today.
    #[arg(default_value = "core")]
    pub name: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct CheckArgs {
    pub site: String,

    /// Print the result as JSON. Implied when stdout is not a terminal.
    #[arg(long)]
    pub json: bool,
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
    TsCli { ir_path: PathBuf },
    Openapi { ir_path: PathBuf },
    Mcp { ir_path: PathBuf },
}

#[derive(Debug, Clone, clap::Args)]
pub struct LintArgs {
    pub ir_path: PathBuf,

    /// Print the result as JSON. Implied when stdout is not a terminal.
    #[arg(long)]
    pub json: bool,
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
        Commands::Emit(args) => emit::run(args).await,
        Commands::Install(args) => install::run(args).await,
        Commands::Lint(args) => lint::run(args),
        Commands::Exec(args) => exec_command(args).await,
        Commands::Auth(args) => auth_command(args).await,
        Commands::Check(args) => check_command(args).await,
        Commands::Shell => shell_command().await,
        Commands::Skills(args) => skills_command(args),
        Commands::Schema => schema_command(),
    }
}

pub async fn exec_command(args: ExecArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let ir_path = surfacer_ir::site_ir_path(&home, &args.site);

    if !ir_path.exists() {
        let registry_path = surfacer_ir::registry_path(&home);
        let registry = surfacer_ir::RegistryIndex::load(&registry_path).unwrap_or_else(|_| surfacer_ir::RegistryIndex { sites: Vec::new() });

        if let Some(entry) = registry.find(&args.site) {
            let descriptor = surfacer_ir::read_ir(&entry.ir_path)
                .with_context(|| format!("failed to read IR for site '{}'", args.site))?;
            return exec_with_ir(&descriptor, &args).await;
        }

        let installed = registry.sites.iter().map(|s| &s.site_name).collect::<Vec<_>>();
        if installed.is_empty() {
            return Err(anyhow!("site '{}' not found. No sites installed.\n\n  Install one with:\n    surfacer install <ir_path>", args.site));
        }
        return Err(anyhow!("site '{}' not found. Installed sites:\n{}\n\n  Install with:\n    surfacer install <ir_path>",
            args.site,
            installed.iter().map(|s| format!("    {s}")).collect::<Vec<_>>().join("\n")
        ));
    }

    let descriptor = surfacer_ir::read_ir(&ir_path)
        .with_context(|| format!("failed to read IR for site '{}'", args.site))?;
    exec_with_ir(&descriptor, &args).await
}

async fn exec_with_ir(descriptor: &surfacer_ir::SiteDescriptor, args: &ExecArgs) -> anyhow::Result<()> {
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
            surfacer_emit_cli::build_help_text_colored(descriptor)
        } else {
            surfacer_emit_cli::build_help_text(descriptor)
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
        let available = surfacer_ir::command_help_rows(descriptor)
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
        surfacer_ir::OperationTransport::Http(http_op) => {
            let endpoint = descriptor.http.as_ref()
                .and_then(|h| h.endpoints.get(http_op.endpoint_index))
                .ok_or_else(|| anyhow!("endpoint index {} out of bounds", http_op.endpoint_index))?;
            let base = url::Url::parse(&descriptor.meta.source_url)
                .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("localhost")))
                .unwrap_or_else(|_| descriptor.meta.source_url.clone());
            format!("{}{}", base, endpoint.path)
        }
        surfacer_ir::OperationTransport::Ax(_) => {
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

pub async fn emit_command(args: EmitArgs) -> anyhow::Result<std::path::PathBuf> {
    let (ir_path, target_label) = match &args.target {
        EmitTargetArg::Cli { ir_path } => (ir_path.clone(), "cli"),
        EmitTargetArg::JustBash { ir_path } => (ir_path.clone(), "just-bash"),
        EmitTargetArg::TsCli { ir_path } => (ir_path.clone(), "ts-cli"),
        EmitTargetArg::Openapi { ir_path } => (ir_path.clone(), "openapi"),
        EmitTargetArg::Mcp { ir_path } => (ir_path.clone(), "mcp"),
    };
    let descriptor = surfacer_ir::read_ir(&ir_path)
        .with_context(|| format!("failed to read IR from {}", ir_path.display()))?;

    if let Err(errors) = surfacer_ir::lint_ir(&descriptor) {
        for error in errors {
            eprintln!("warning: {error}");
        }
    }

    // Targets that write a single generated file share everything but the
    // emitter, the extension, and what to tell the user afterwards.
    let single_file: Option<(&str, fn(&surfacer_ir::SiteDescriptor) -> String, &str)> =
        match target_label {
            "ts-cli" => Some(("ts", surfacer_emit_cli::emit_ts_cli, "TypeScript CLI")),
            "openapi" => Some((
                "openapi.json",
                surfacer_emit_cli::emit_openapi,
                "OpenAPI document",
            )),
            "mcp" => Some(("mcp.js", surfacer_emit_cli::emit_mcp_server, "MCP server")),
            _ => None,
        };

    if let Some((extension, emit, label)) = single_file {
        let out_dir = args.out_dir.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("emit")
                .join(target_label)
        });
        std::fs::create_dir_all(&out_dir)
            .with_context(|| format!("failed to create {}", out_dir.display()))?;

        let site = &descriptor.meta.site_name;
        let source = emit(&descriptor);
        let source_path = out_dir.join(format!("{site}.{extension}"));
        std::fs::write(&source_path, &source)
            .with_context(|| format!("failed to write {}", source_path.display()))?;

        ok(&format!(
            "{label} written: {} ({}B)",
            source_path.display(),
            source.len()
        ));
        eprintln!();

        match target_label {
            "ts-cli" => {
                hint("Run it directly:");
                cmd(&format!("scriptc run {} -- --help", source_path.display()));
                eprintln!();
                hint("Or compile a native binary (no runtime required):");
                cmd(&format!(
                    "scriptc build {} --dynamic -o {site}",
                    source_path.display()
                ));
            }
            "openapi" => {
                hint("Feed it to any OpenAPI consumer:");
                cmd(&format!("npx @stainless-api/cli generate {}", source_path.display()));
                cmd(&format!("restish api configure {site} {}", source_path.display()));
            }
            "mcp" => {
                hint("Install its dependencies and register it:");
                cmd("npm install @modelcontextprotocol/server zod");
                cmd(&format!(
                    "claude mcp add {site} -- node {}",
                    source_path.display()
                ));
            }
            _ => {}
        }

        return Ok(source_path);
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

        let config_content = surfacer_emit_cli::emit_executor_config(&descriptor);
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
            "import {{ createSurfacerExecutor }} from \"./{}.config.js\";",
            descriptor.meta.site_name
        ));
        cmd("const executor = await createSurfacerExecutor(createExecutor);");
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
    let emitted = surfacer_emit_cli::emit_cli_shim(surfacer_emit_cli::CliEmitRequest {
        descriptor,
        out_dir,
    })?;

    ok(&format!("Shim compiled: {} ({}KB)",
        emitted.binary_path.display(),
        emitted.binary_size / 1024
    ));
    eprintln!();
    hint("Next step:");
    cmd(&format!("surfacer install {}", ir_path.display()));

    Ok(emitted.binary_path)
}

pub async fn install_command(args: InstallArgs) -> anyhow::Result<InstallSuccessView> {
    let source_path = resolve_local_ir_source(&args.source)?;
    let source_label = source_path.display().to_string();

    let descriptor = surfacer_ir::read_ir(&source_path)
        .with_context(|| format!("failed to read IR from {}", source_path.display()))?;
    let progress = InstallProgressView {
        source_label,
        version_label: descriptor.meta.ir_version.clone(),
    };
    render_install_progress(&progress);

    if let Err(errors) = surfacer_ir::lint_ir(&descriptor) {
        for error in errors {
            println!("warning: {error}");
        }
    }

    let home_dir = home_dir()?;
    let site_ir_path = surfacer_ir::site_ir_path(&home_dir, &descriptor.meta.site_name);
    surfacer_ir::write_ir(&site_ir_path, &descriptor)
        .with_context(|| format!("failed to copy IR to {}", site_ir_path.display()))?;

    let shim_build_dir = std::env::temp_dir().join(format!(
        "surfacer-install-{}-{}",
        descriptor.meta.site_name,
        std::process::id()
    ));
    let emitted = surfacer_emit_cli::emit_cli_shim(surfacer_emit_cli::CliEmitRequest {
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

    let registry_path = surfacer_ir::registry_path(&home_dir);
    let mut registry = surfacer_ir::RegistryIndex::load(&registry_path)
        .with_context(|| format!("failed to load registry at {}", registry_path.display()))?;
    registry.upsert(surfacer_ir::InstalledSiteEntry {
        site_name: descriptor.meta.site_name.clone(),
        ir_path: site_ir_path.clone(),
        shim_path: installed_shim_path.clone(),
    });
    registry
        .save(&registry_path)
        .with_context(|| format!("failed to write registry at {}", registry_path.display()))?;

    let meta_path = surfacer_ir::site_meta_path(&home_dir, &descriptor.meta.site_name);
    if let Some(parent) = meta_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create site metadata dir {}", parent.display()))?;
    }
    let install_record = surfacer_ir::InstallRecord {
        site_name: descriptor.meta.site_name.clone(),
        ir_path: site_ir_path,
        shim_path: installed_shim_path,
        installed_at: timestamp_string()?,
        source: surfacer_ir::InstallSource::LocalPath(surfacer_ir::LocalPathSource {
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
    descriptor: &surfacer_ir::SiteDescriptor,
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
        surfacer_ir::OperationTransport::Http(http_op) => {
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
    let ir_path = surfacer_ir::site_ir_path(&home, &args.site);
    if !ir_path.exists() {
        return Err(anyhow!("site '{}' not installed. Run: surfacer install <ir_path>", args.site));
    }

    let descriptor = surfacer_ir::read_ir(&ir_path)?;
    let login_url = args.url.clone().unwrap_or_else(|| descriptor.meta.source_url.clone());
    let session_name = format!("surfacer-{}", args.site);

    step(&format!("Opening {} for authentication...", login_url));
    hint("Log in normally in the browser window.");
    hint("Press ENTER here when you're logged in.");

    let browser = surfacer_probe::agent_browser::BrowserProcess {
        child_id: 0,
        cdp_port: 9222,
        profile_dir: std::path::PathBuf::new(),
    };
    let probe_opts = surfacer_probe::ProbeOptions {
        url: login_url.clone(),
        visible: true,
        output_dir: std::env::temp_dir().join(format!("surfacer-auth-{}", args.site)),
    };
    let session = surfacer_probe::agent_browser::connect_session(browser, &probe_opts).await
        .context("failed to connect to browser. Is Comet running with --remote-debugging-port=9222?")?;

    ok(&format!("Browser opened: {}", login_url));

    // If the descriptor wants a browser-bootstrapped token, record the login
    // navigation into a HAR so the token can be pulled from it after ENTER.
    let token_auth = find_browser_token_auth(&descriptor);
    if token_auth.is_some() {
        surfacer_probe::agent_browser::start_har_capture(&session).await
            .context("failed to start HAR capture for token login")?;
    }

    eprintln!();
    step("Waiting for you to log in... press ENTER when done.");
    wait_for_enter().await?;

    let session_dir = surfacer_ir::site_dir(&home, &args.site);
    std::fs::create_dir_all(&session_dir)?;
    let session_file = session_dir.join("session-name");
    std::fs::write(&session_file, &session_name)?;

    if let Some(auth) = token_auth {
        capture_and_store_token(&session, auth, &session_dir).await?;
    }

    ok(&format!("Session saved for '{}'. Commands will now use authenticated state.", args.site));
    hint(&format!("Try: {} --help", args.site));

    Ok(())
}

/// Find the first `BrowserBootstrappedToken` auth in the descriptor, checking
/// the HTTP surface default first, then per-operation overrides. `auth login`
/// only needs the token capture spec, which is site-wide in practice.
fn find_browser_token_auth(
    descriptor: &surfacer_ir::SiteDescriptor,
) -> Option<&surfacer_ir::BrowserBootstrappedTokenAuth> {
    if let Some(surfacer_ir::AuthMode::BrowserBootstrappedToken(auth)) =
        descriptor.http.as_ref().and_then(|h| h.auth.as_ref())
    {
        return Some(auth);
    }
    descriptor.operations.iter().find_map(|op| match op.auth.as_ref() {
        Some(surfacer_ir::AuthMode::BrowserBootstrappedToken(auth)) => Some(auth),
        _ => None,
    })
}

/// Stop the HAR, extract the token per the capture spec, decode its expiry, and
/// persist it as `token.json` next to `session-name`. Ports the proven logic in
/// crafter-research/sunat-cli/packages/cli/src/plataforma/session.ts.
async fn capture_and_store_token(
    session: &surfacer_probe::ProbeSession,
    auth: &surfacer_ir::BrowserBootstrappedTokenAuth,
    session_dir: &Path,
) -> anyhow::Result<()> {
    let har_path = surfacer_probe::agent_browser::stop_har_capture(session).await
        .context("failed to stop HAR capture for token login")?;
    let bytes = std::fs::read(&har_path)
        .with_context(|| format!("failed to read captured HAR at {}", har_path.display()))?;
    let har = surfacer_probe::har::parse_har(&bytes)?;

    let token = surfacer_probe::extract_token(&har, &auth.acquire.capture)?
        .ok_or_else(|| anyhow!(
            "no token found in the login network log. Navigate into the authenticated form first, then press ENTER (the token only appears once the form loads)."
        ))?;

    let expires_at = crate::token_cache::decode_exp(&token)?;
    let captured_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock before unix epoch")?
        .as_secs();
    let cache = crate::token_cache::TokenCache {
        token,
        expires_at,
        captured_at,
    };

    let token_file = session_dir.join("token.json");
    std::fs::write(&token_file, serde_json::to_string_pretty(&cache)?)
        .with_context(|| format!("failed to write {}", token_file.display()))?;

    ok(&format!("Token captured (expires at {expires_at})."));
    Ok(())
}

async fn auth_status(args: AuthStatusArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let session_file = surfacer_ir::site_dir(&home, &args.site).join("session-name");

    if session_file.exists() {
        let session = std::fs::read_to_string(&session_file)?;
        ok(&format!("Site '{}' has authenticated session: {}", args.site, session));
    } else {
        warn(&format!("Site '{}' has no authenticated session.", args.site));
        hint(&format!("Run: surfacer auth login {}", args.site));
    }
    Ok(())
}

async fn auth_logout(args: AuthLogoutArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let session_file = surfacer_ir::site_dir(&home, &args.site).join("session-name");

    if session_file.exists() {
        std::fs::remove_file(&session_file)?;
        ok(&format!("Logged out of '{}'.", args.site));
    } else {
        hint(&format!("Site '{}' was not logged in.", args.site));
    }
    Ok(())
}

/// This CLI's own command surface, as data.
///
/// An agent reads this instead of parsing `--help`, which is prose written for
/// a person and changes shape without notice. `version` lets a caller detect
/// that the surface it learned is no longer the one installed.
fn schema_command() -> anyhow::Result<()> {
    emit_json(&serde_json::json!({
        "name": "surfacer",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Compile an IR describing a surface into working interfaces",
        "exitCodes": {
            "0": "success",
            "1": "user error, the request itself was wrong",
            "2": "system error, the filesystem or network failed",
        },
        "commands": [
            {
                "name": "lint",
                "summary": "Validate an IR before anything downstream reads it",
                "args": [{ "name": "ir_path", "required": true }],
                "flags": ["--json"],
            },
            {
                "name": "install",
                "summary": "Register an IR under its site name",
                "args": [{ "name": "source", "required": true }],
                "flags": ["--dest"],
            },
            {
                "name": "emit",
                "summary": "Generate an interface from an IR",
                "args": [{ "name": "target", "required": true,
                           "values": ["cli", "ts-cli", "just-bash", "openapi", "mcp"] },
                         { "name": "ir_path", "required": true }],
                "flags": ["--out-dir"],
            },
            {
                "name": "exec",
                "summary": "Run an operation from an installed site",
                "args": [{ "name": "site", "required": true }],
            },
            {
                "name": "check",
                "summary": "Detect whether an installed site drifted from its IR",
                "args": [{ "name": "site", "required": true }],
                "flags": ["--json"],
            },
            {
                "name": "auth",
                "summary": "Capture and manage a session for an authenticated site",
                "subcommands": ["login", "status", "logout"],
            },
            {
                "name": "skills",
                "summary": "Serve the agent-facing manual embedded in this binary",
                "subcommands": ["list", "get"],
            },
            {
                "name": "schema",
                "summary": "Print this command surface as JSON",
            },
        ],
    }));
    Ok(())
}

/// The agent-facing manual, embedded at build time.
///
/// Serving it from the binary rather than shipping a copy elsewhere is the
/// point: a manual kept in a second place drifts from the version installed,
/// and the reader has no way to tell it went stale. This one is always what
/// the running binary was built from.
const SKILL_CORE: &str = include_str!("../../../skills/surfacer/SKILL.md");

fn skills_command(args: SkillsArgs) -> anyhow::Result<()> {
    match args.action {
        SkillsAction::List => {
            // The listing is a diagnostic; only a requested document is data.
            eprintln!("core    How surfacer works: the loop, the six targets, drift, auth");
            hint("Print one with: surfacer skills get core");
            Ok(())
        }
        SkillsAction::Get(get) => match get.name.as_str() {
            "core" => {
                println!("{SKILL_CORE}");
                Ok(())
            }
            other => Err(anyhow!(
                "unknown skill '{other}'. Run `surfacer skills list` to see what exists."
            )),
        },
    }
}

/// Detect whether an installed site drifted out from under its IR.
///
/// Known limitation: this only covers `descriptor.http`. It takes up to three
/// endpoints as canaries and fingerprints each with a HEAD-style request, so
/// the whole check is an HTTP reachability comparison.
///
/// An IR for a non-HTTP terrain (a file format, a hardware interface, a pure
/// AX surface) has no `http` surface to sample, so it exits early with "no
/// endpoints to check" and the drift check tells that site nothing. That is a
/// known limitation, not a bug: the canary technique does not generalize to
/// terrains where there is no endpoint to probe. Those terrains need their own
/// drift signal (a format version, a firmware revision, an AX tree hash), which
/// this command does not implement.
pub async fn check_command(args: CheckArgs) -> anyhow::Result<()> {
    let home = home_dir()?;
    let ir_path = surfacer_ir::site_ir_path(&home, &args.site);
    if !ir_path.exists() {
        return Err(anyhow!("site '{}' not installed", args.site));
    }

    let descriptor = surfacer_ir::read_ir(&ir_path)?;
    let fingerprint_path = surfacer_ir::site_dir(&home, &args.site).join("fingerprint.json");

    let base = url::Url::parse(&descriptor.meta.source_url)
        .map(|u| format!("{}://{}", u.scheme(), u.host_str().unwrap_or("localhost")))
        .unwrap_or_else(|_| descriptor.meta.source_url.clone());

    let canaries: Vec<&surfacer_ir::HttpEndpoint> = descriptor
        .http
        .as_ref()
        .map(|h| h.endpoints.iter().take(3).collect())
        .unwrap_or_default();

    let mode = crate::output::Mode::resolve(args.json);

    if canaries.is_empty() {
        if mode.is_json() {
            // A terrain with no endpoints is not an error, it is a site this
            // check cannot speak about. Saying so as data keeps a caller from
            // reading silence as "no drift".
            emit_json(&serde_json::json!({
                "site": args.site,
                "checked": false,
                "reason": "no HTTP endpoints to check drift against",
                "technique": descriptor.provenance.technique,
            }));
            return Ok(());
        }
        warn("No HTTP endpoints to check drift against.");
        return Ok(());
    }

    if !mode.is_json() {
        step(&format!("Checking {} canary endpoints for {}...", canaries.len(), args.site));
    }

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
        if mode.is_json() {
            emit_json(&serde_json::json!({
                "site": args.site,
                "checked": true,
                "baseline": true,
                "drifted": [],
                "endpoints": current.len(),
                "technique": descriptor.provenance.technique,
            }));
            return Ok(());
        }
        ok(&format!("Baseline fingerprint saved ({} endpoints)", current.len()));
        hint("Run 'surfacer check' again later to detect drift.");
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

    if mode.is_json() {
        // `fingerprintUpdated` is not decoration: when drift is found the
        // baseline is rewritten, so the next run reports clean whether or not
        // anything was fixed. A caller polling this command has to know that
        // this result is the one that carried the signal.
        let updated = !drifted.is_empty();
        if updated {
            let json = serde_json::to_string_pretty(&current)?;
            std::fs::write(&fingerprint_path, &json)?;
        }
        emit_json(&serde_json::json!({
            "site": args.site,
            "checked": true,
            "baseline": false,
            "drifted": drifted,
            "matched": matched,
            "endpoints": current.len(),
            "technique": descriptor.provenance.technique,
            "sourceUrl": descriptor.meta.source_url,
            "fingerprintUpdated": updated,
        }));
        return Ok(());
    }

    if drifted.is_empty() {
        ok(&format!("No drift detected ({}/{} endpoints match)", matched, current.len()));
    } else {
        warn(&format!("Drift detected in {} endpoint(s):", drifted.len()));
        for path in &drifted {
            hint(&format!("  {path}"));
        }
        eprintln!();
        // How to refresh the IR depends on where it came from. Telling the
        // author of an agent-made IR to regenerate it mechanically would throw
        // away the judgment that produced it, so the two cases differ.
        match descriptor.provenance.technique {
            surfacer_ir::ProvenanceTechnique::Agent => {
                hint("The site may have changed. Re-run the recon skill on it and reinstall:");
                cmd(&format!("/surfacer {}", descriptor.meta.source_url));
                cmd("surfacer install <new-ir-path>");
            }
            _ => {
                hint("The site may have changed. Replace the IR with an updated one:");
                cmd("surfacer install <new-ir-path>");
            }
        }

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
        let state_path = surfacer_ir::site_dir(&std::path::PathBuf::from(home), site_name)
            .join("last-command");
        let _ = std::fs::write(state_path, command);
    }
}

fn load_last_command(site_name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let state_path = surfacer_ir::site_dir(&std::path::PathBuf::from(home), site_name)
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

#[cfg(test)]
mod auth_login_tests {
    use super::find_browser_token_auth;
    use surfacer_ir::{
        AuthMode, BrowserAcquisition, BrowserBootstrappedTokenAuth, CredentialLocation,
        HttpSurface, Provenance, ProvenanceTechnique, RenewalStrategy, SiteDescriptor, SiteMeta,
        TokenCapture, TokenTtl, TokenUse,
    };

    fn descriptor_with_surface_auth(auth: Option<AuthMode>) -> SiteDescriptor {
        SiteDescriptor {
            meta: SiteMeta {
                site_name: "sunat".into(),
                display_name: "SUNAT".into(),
                source_url: "https://www.sunat.gob.pe".into(),
                ir_version: "0.1.0".into(),
            },
            provenance: Provenance {
                generated_at: "2026-08-09T00:00:00Z".into(),
                technique: ProvenanceTechnique::Http,
                classifier_bucket: "FormSessionLegacy".into(),
                probe_duration_sec: 1,
            },
            operations: Vec::new(),
            http: Some(HttpSurface {
                endpoints: Vec::new(),
                auth,
            }),
            ax: None,
        }
    }

    fn browser_token_mode() -> AuthMode {
        AuthMode::BrowserBootstrappedToken(BrowserBootstrappedTokenAuth {
            acquire: BrowserAcquisition {
                login_url: "https://e-menu.sunat.gob.pe/login".into(),
                capture: TokenCapture::RequestQueryParam {
                    url_contains: "servletAcceso".into(),
                    param: "idCache".into(),
                },
                session_ref: Some("sunat".into()),
            },
            use_: TokenUse {
                location: CredentialLocation::Header,
                name: "IdCache".into(),
                value_prefix: None,
            },
            ttl: TokenTtl {
                seconds: 3600,
                on_expiry: RenewalStrategy::PromptReauth,
            },
        })
    }

    #[test]
    fn finds_browser_token_auth_on_surface() {
        let descriptor = descriptor_with_surface_auth(Some(browser_token_mode()));
        let found = find_browser_token_auth(&descriptor).expect("expected browser token auth");
        assert!(matches!(
            found.acquire.capture,
            TokenCapture::RequestQueryParam { .. }
        ));
    }

    #[test]
    fn descriptor_without_browser_auth_does_not_capture() {
        // A public surface (auth None) must return None, so auth login never
        // starts HAR capture or looks for a token.
        let descriptor = descriptor_with_surface_auth(Some(AuthMode::None));
        assert!(find_browser_token_auth(&descriptor).is_none());

        // And a descriptor with no surface auth at all.
        let bare = descriptor_with_surface_auth(None);
        assert!(find_browser_token_auth(&bare).is_none());
    }
}
