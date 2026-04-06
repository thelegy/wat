mod backend;
pub mod cli;
mod operations;

use std::{path::Path, sync::Once};

use anyhow::{bail, Result};
use clap::Parser;

static INIT_LOGGER: Once = Once::new();

pub fn run() -> Result<()> {
    init_logging();
    let cli = cli::Cli::parse();
    match cli.command {
        cli::Command::Completion(args) => {
            cli::print_completions(args.shell);
            Ok(())
        }
        command => {
            prepare_environment()?;
            operations::execute(command)
        }
    }
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
