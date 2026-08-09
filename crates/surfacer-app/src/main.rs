use clap::Parser;
use tracing_subscriber::fmt::init;

mod cli;
mod commands;
mod execute;
mod extract;
mod shell;
mod ui;

pub use cli::*;

#[tokio::main]
async fn main() {
    init();
    let cli = Cli::parse();
    if let Err(e) = run_cli(cli).await {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
