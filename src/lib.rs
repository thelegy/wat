mod backend;
pub mod cli;
mod config;
mod event;
mod operations;
mod planner;
mod recording;
mod reducer;
mod reporter;

use std::{path::Path, sync::Once};

use anyhow::{Result, bail};
use clap::Parser;

static INIT_LOGGER: Once = Once::new();

pub async fn run() -> Result<()> {
    init_logging();
    let cli = cli::Cli::parse();
    if let cli::Command::Replay { path } = &cli.command {
        return recording::replay(path).await;
    }
    prepare_environment()?;
    recording::init()?;
    let result = operations::execute(cli.command).await;
    recording::write(
        serde_json::json!({"kind": "outcome", "error": result.as_ref().err().map(|error| format!("{error}"))}),
    );
    result
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
