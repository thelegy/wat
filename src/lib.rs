mod backend;
pub mod cli;
mod operations;

use std::{path::Path, sync::Once};

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;

static INIT_LOGGER: Once = Once::new();

pub fn run() -> Result<()> {
    CompleteEnv::with_factory(cli::Cli::command).complete();
    init_logging();
    let cli = cli::Cli::parse();
    prepare_environment()?;
    operations::execute(cli.command)
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
