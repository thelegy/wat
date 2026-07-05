mod event;
mod backend;
pub mod cli;
mod operations;
mod planner;
mod reducer;
mod reporter;

use std::{path::Path, sync::Once};

use anyhow::{Result, bail};
use clap::Parser;

static INIT_LOGGER: Once = Once::new();

pub async fn run() -> Result<()> {
    init_logging();
    let cli = cli::Cli::parse();
    prepare_environment()?;
    operations::execute(cli.command).await
}

pub fn complete() {
    use clap::CommandFactory;
    use clap_complete::CompleteEnv;

    CompleteEnv::with_factory(cli::Cli::command).complete();
}

fn init_logging() {
    INIT_LOGGER.call_once(|| {
        let env = env_logger::Env::default().default_filter_or("info");
        if let Err(err) = env_logger::Builder::from_env(env).try_init() {
            eprintln!("failed to initialize logger: {err}");
        }
    });
}

fn prepare_environment() -> Result<()> {
    if !Path::new("flake.nix").exists() {
        log::error!("Error: ./flake.nix not found in current directory");
        bail!("flake.nix missing");
    }

    Ok(())
}
