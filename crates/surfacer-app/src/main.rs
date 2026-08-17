use clap::Parser;
use tracing_subscriber::fmt::init;

mod cli;
mod commands;
mod execute;
mod extract;
mod output;
mod shell;
mod token_cache;
mod ui;

pub use cli::*;

#[tokio::main]
async fn main() {
    init();
    let cli = Cli::parse();
    if let Err(e) = run_cli(cli).await {
        // Errors are diagnostics, so they never touch stdout: a caller parsing
        // this command's JSON must not have to filter a failure out of it.
        eprintln!("error: {e:#}");
        std::process::exit(output::exit_code_for(&e));
    }
}
