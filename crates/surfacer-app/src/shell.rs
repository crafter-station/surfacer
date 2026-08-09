use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::Context;
use owo_colors::OwoColorize;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config, Editor, Helper};

use crate::cli::{CheckArgs, ExecArgs};

struct ShellState {
    current_site: Option<String>,
    current_descriptor: Option<surfacer_ir::SiteDescriptor>,
    home: PathBuf,
    site_names: Vec<String>,
}

#[derive(Clone)]
struct ShellCompleter {
    site_names: Vec<String>,
    site_commands: Vec<String>,
    shell_commands: Vec<String>,
    in_site: bool,
}

impl ShellCompleter {
    fn new(site_names: &[String]) -> Self {
        Self {
            site_names: site_names.clone().to_vec(),
            site_commands: Vec::new(),
            shell_commands: vec![
                "ls".into(), "list".into(), "open".into(), "switch".into(),
                "back".into(), "close".into(), "check".into(), "help".into(),
                "exit".into(), "quit".into(), "clear".into(),
            ],
            in_site: false,
        }
    }

    fn enter_site(&mut self, descriptor: &surfacer_ir::SiteDescriptor) {
        self.site_commands = surfacer_ir::command_help_rows(descriptor)
            .iter()
            .map(|r| r.command.clone())
            .collect();
        self.site_commands.push("--help".into());
        self.site_commands.push("--json".into());
        self.site_commands.push("open".into());
        self.in_site = true;
    }

    fn leave_site(&mut self) {
        self.site_commands.clear();
        self.in_site = false;
    }
}

impl Completer for ShellCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let input = &line[..pos];
        let parts: Vec<&str> = input.split_whitespace().collect();

        let (prefix, candidates) = if parts.is_empty() || (parts.len() == 1 && !input.ends_with(' ')) {
            let prefix = parts.first().copied().unwrap_or("");
            let mut candidates: Vec<&str> = self.shell_commands.iter().map(|s| s.as_str()).collect();
            if self.in_site {
                candidates.extend(self.site_commands.iter().map(|s| s.as_str()));
            } else {
                candidates.extend(self.site_names.iter().map(|s| s.as_str()));
            }
            (prefix, candidates)
        } else if self.in_site && parts.len() >= 1 {
            let prefix = if input.ends_with(' ') { "" } else { parts.last().copied().unwrap_or("") };
            let candidates: Vec<&str> = self.site_commands.iter().map(|s| s.as_str()).collect();
            (prefix, candidates)
        } else {
            let prefix = if input.ends_with(' ') { "" } else { parts.last().copied().unwrap_or("") };
            let first = parts[0];
            if first == "open" || first == "switch" {
                (prefix, self.site_names.iter().map(|s| s.as_str()).collect())
            } else {
                (prefix, Vec::new())
            }
        };

        let start = pos - prefix.len();
        let matches: Vec<Pair> = candidates
            .iter()
            .filter(|c| c.starts_with(prefix))
            .map(|c| Pair {
                display: c.to_string(),
                replacement: c.to_string(),
            })
            .collect();

        Ok((start, matches))
    }
}

impl Hinter for ShellCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        if pos < line.len() {
            return None;
        }
        let input = line.trim();
        if input.is_empty() {
            return None;
        }

        let candidates: Vec<&str> = if self.in_site {
            self.site_commands.iter().map(|s| s.as_str()).collect()
        } else {
            let mut c: Vec<&str> = self.shell_commands.iter().map(|s| s.as_str()).collect();
            c.extend(self.site_names.iter().map(|s| s.as_str()));
            c
        };

        let parts: Vec<&str> = input.split_whitespace().collect();
        let last = parts.last().copied().unwrap_or("");

        candidates
            .iter()
            .find(|c| c.starts_with(last) && **c != last)
            .map(|c| c[last.len()..].to_string())
    }
}

impl Highlighter for ShellCompleter {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("{}", hint.dimmed()))
    }
}

impl Validator for ShellCompleter {}
impl Helper for ShellCompleter {}

fn history_path(home: &std::path::Path, site: Option<&str>) -> PathBuf {
    let dir = home.join(".surfacer").join("history");
    let _ = std::fs::create_dir_all(&dir);
    match site {
        Some(name) => dir.join(format!("{name}.history")),
        None => dir.join("global.history"),
    }
}

pub async fn run_shell() -> anyhow::Result<()> {
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .context("HOME not set")?;

    let registry_path = surfacer_ir::registry_path(&home);
    let registry = surfacer_ir::RegistryIndex::load(&registry_path)
        .unwrap_or_else(|_| surfacer_ir::RegistryIndex { sites: Vec::new() });

    let site_names: Vec<String> = registry.sites.iter().map(|s| s.site_name.clone()).collect();

    let mut state = ShellState {
        current_site: None,
        current_descriptor: None,
        home: home.clone(),
        site_names: site_names.clone(),
    };

    let completer = ShellCompleter::new(&site_names);

    let config = Config::builder()
        .auto_add_history(true)
        .build();

    let mut rl = Editor::with_config(config).context("failed to init readline")?;
    rl.set_helper(Some(completer));

    let global_history = history_path(&home, None);
    let _ = rl.load_history(&global_history);

    print_welcome(&state);

    loop {
        let prompt = build_prompt(&state);
        let line = match rl.readline(&prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => return Err(e.into()),
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "exit" | "quit" | "q" => break,
            "help" | "?" => print_shell_help(&state),
            "ls" | "list" => list_sites(&state),
            "clear" => {
                let _ = rl.clear_screen();
            }
            "open" => {
                if args.is_empty() {
                    eprintln!("  usage: open <site>");
                } else if state.current_site.is_some() && args[0].parse::<usize>().is_ok() {
                    if let Some(ref site) = state.current_site {
                        let mut a = vec!["open"];
                        a.extend_from_slice(args);
                        dispatch_site_command(site, &a, &state).await;
                    }
                } else {
                    open_site(args[0], &mut state, &mut rl, &home);
                }
            }
            "switch" => {
                if args.is_empty() {
                    eprintln!("  usage: switch <site>");
                } else {
                    open_site(args[0], &mut state, &mut rl, &home);
                }
            }
            "back" | "close" => {
                let _ = rl.save_history(&history_path(&home, state.current_site.as_deref()));
                state.current_site = None;
                state.current_descriptor = None;
                if let Some(h) = rl.helper_mut() {
                    h.leave_site();
                }
                let _ = rl.load_history(&global_history);
            }
            "check" => {
                if let Some(ref site) = state.current_site {
                    let _ = crate::check_command(CheckArgs { site: site.clone() }).await;
                } else if !args.is_empty() {
                    let _ = crate::check_command(CheckArgs { site: args[0].to_string() }).await;
                } else {
                    eprintln!("  usage: check <site>");
                }
            }
            _ => {
                if let Some(ref site) = state.current_site {
                    dispatch_site_command(site, &parts, &state).await;
                } else {
                    if state.site_names.contains(&cmd.to_string()) {
                        open_site(cmd, &mut state, &mut rl, &home);
                        if !args.is_empty() {
                            let site = state.current_site.as_ref().unwrap().clone();
                            dispatch_site_command(&site, args, &state).await;
                        }
                    } else {
                        eprintln!("  unknown command: {cmd}. Type 'help' for options.");
                    }
                }
            }
        }
    }

    let hist_path = history_path(&home, state.current_site.as_deref());
    let _ = rl.save_history(&hist_path);

    eprintln!("goodbye");
    Ok(())
}

fn open_site(name: &str, state: &mut ShellState, rl: &mut Editor<ShellCompleter, rustyline::history::DefaultHistory>, home: &std::path::Path) {
    let ir_path = surfacer_ir::site_ir_path(home, name);
    if !ir_path.exists() {
        eprintln!("  site '{name}' not found");
        return;
    }

    if let Some(ref prev_site) = state.current_site {
        let _ = rl.save_history(&history_path(home, Some(prev_site)));
    } else {
        let _ = rl.save_history(&history_path(home, None));
    }

    match surfacer_ir::read_ir(&ir_path) {
        Ok(desc) => {
            let cmd_count = desc.operations.len();
            if let Some(h) = rl.helper_mut() {
                h.enter_site(&desc);
            }
            let _ = rl.load_history(&history_path(home, Some(name)));
            state.current_site = Some(name.to_string());
            state.current_descriptor = Some(desc);

            if crate::execute::use_color() {
                eprintln!("  {} ({} commands)", name.cyan().bold(), cmd_count);
            } else {
                eprintln!("  {name} ({cmd_count} commands)");
            }
        }
        Err(e) => {
            eprintln!("  failed to load site '{name}': {e}");
        }
    }
}

fn print_welcome(state: &ShellState) {
    let registry_path = surfacer_ir::registry_path(&state.home);
    let registry = surfacer_ir::RegistryIndex::load(&registry_path)
        .unwrap_or_else(|_| surfacer_ir::RegistryIndex { sites: Vec::new() });

    if crate::execute::use_color() {
        eprintln!("{}", "surfacer shell".bold());
        eprintln!("{}", format!("{} sites installed", registry.sites.len()).dimmed());
        eprintln!("{}", "Type 'help' for commands, 'ls' to list sites, 'exit' to quit.".dimmed());
    } else {
        eprintln!("surfacer shell");
        eprintln!("{} sites installed", registry.sites.len());
        eprintln!("Type 'help' for commands, 'ls' to list sites, 'exit' to quit.");
    }
    eprintln!();
}

fn build_prompt(state: &ShellState) -> String {
    if crate::execute::use_color() {
        match &state.current_site {
            Some(site) => format!("{} {} ", site.cyan(), ">".dimmed()),
            None => format!("{} {} ", "surfacer".bold(), ">".dimmed()),
        }
    } else {
        match &state.current_site {
            Some(site) => format!("{site} > "),
            None => "surfacer > ".to_string(),
        }
    }
}

fn print_shell_help(state: &ShellState) {
    let color = crate::execute::use_color();
    eprintln!();
    if color {
        eprintln!("  {}", "Shell commands:".dimmed());
        eprintln!("    {}          {}", "ls".green(), "List installed sites".dimmed());
        eprintln!("    {}  {}", "open <site>".green(), "Enter a site context".dimmed());
        eprintln!("    {}    {}", "switch <s>".green(), "Switch to another site".dimmed());
        eprintln!("    {}        {}", "back".green(), "Leave current site context".dimmed());
        eprintln!("    {}   {}", "check [site]".green(), "Check for drift".dimmed());
        eprintln!("    {}       {}", "clear".green(), "Clear screen".dimmed());
        eprintln!("    {}        {}", "exit".green(), "Quit shell".dimmed());
    } else {
        eprintln!("  Shell commands:");
        eprintln!("    ls          List installed sites");
        eprintln!("    open <site> Enter a site context");
        eprintln!("    switch <s>  Switch to another site");
        eprintln!("    back        Leave current site context");
        eprintln!("    check       Check for drift");
        eprintln!("    clear       Clear screen");
        eprintln!("    exit        Quit shell");
    }

    if let Some(ref site) = state.current_site {
        if let Some(ref desc) = state.current_descriptor {
            eprintln!();
            if color {
                eprintln!("  {} ({}):", format!("{site} commands").dimmed(), desc.operations.len());
            } else {
                eprintln!("  {site} commands ({}):", desc.operations.len());
            }
            for row in surfacer_ir::command_help_rows(desc).iter().take(8) {
                if color {
                    eprintln!("    {}  {}", row.command.green(), row.description.dimmed());
                } else {
                    eprintln!("    {}  {}", row.command, row.description);
                }
            }
            if desc.operations.len() > 8 {
                eprintln!("    ... and {} more", desc.operations.len() - 8);
            }
        }
    }
    eprintln!();
}

fn list_sites(state: &ShellState) {
    let registry_path = surfacer_ir::registry_path(&state.home);
    let registry = surfacer_ir::RegistryIndex::load(&registry_path)
        .unwrap_or_else(|_| surfacer_ir::RegistryIndex { sites: Vec::new() });

    if registry.sites.is_empty() {
        eprintln!("  No sites installed. Run: surfacer recon <url> --auto --yes");
        return;
    }

    let color = crate::execute::use_color();
    for entry in &registry.sites {
        let cmd_count = surfacer_ir::read_ir(&entry.ir_path)
            .map(|d| d.operations.len())
            .unwrap_or(0);

        let active = state.current_site.as_deref() == Some(&entry.site_name);
        let marker = if active { "→ " } else { "  " };

        if color {
            let name = if active {
                format!("{}", entry.site_name.cyan().bold())
            } else {
                format!("{}", entry.site_name.white())
            };
            eprintln!("{marker}{name}  {}", format!("{cmd_count} commands").dimmed());
        } else {
            eprintln!("{marker}{}  {cmd_count} commands", entry.site_name);
        }
    }
}

async fn dispatch_site_command(site: &str, parts: &[&str], _state: &ShellState) {
    let args_vec: Vec<String> = parts.iter().map(|s| s.to_string()).collect();

    let exec_args = ExecArgs {
        site: site.to_string(),
        args: args_vec,
    };

    if let Err(e) = crate::exec_command(exec_args).await {
        eprintln!("  error: {e:#}");
    }
}
