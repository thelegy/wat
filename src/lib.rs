mod backend;
pub mod cli;
mod logging;
mod operations;

use std::path::Path;

use anyhow::{Result, bail};
use clap::Parser;

pub fn run() -> Result<()> {
    prepare_environment()?;
    let cli = cli::Cli::parse();
    operations::execute(cli.command)
}

fn prepare_environment() -> Result<()> {
    if !Path::new("flake.nix").exists() {
        logging::print_error("Error: ./flake.nix not found in current directory");
        bail!("flake.nix missing");
    }

    Ok(())
}

pub use logging::{print_error, print_info, print_warning};
