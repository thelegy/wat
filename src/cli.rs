use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use clap_complete::{ArgValueCandidates, engine::CompletionCandidate};
use log::debug;

use crate::backend;

#[derive(Debug, Parser)]
#[command(name = "wat", about = "Deploy tool for NixOS hosts")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build, switch, and test configuration on target host
    Switch(CommonBuildArgs),

    /// Build and activate configuration without running tests
    Boot(CommonBuildArgs),

    /// Boot configuration and reboot to activate
    Reboot(CommonBuildArgs),

    /// Build and run nixos-rebuild test
    Test(CommonBuildArgs),

    /// Build and perform dry activation
    DryActivate(CommonBuildArgs),

    /// Only build the configuration and print store path
    Build(CommonBuildArgs),

    /// Build ISO image and print store path
    Iso(CommonBuildArgs),

    /// Build SD card image and print store path
    Sdcard(CommonBuildArgs),
}

#[derive(Debug, Args)]
pub struct CommonBuildArgs {
    /// The names of the hosts to operate on
    #[arg(required = true, num_args = 1.., add = ArgValueCandidates::new(fetch_hostname_candidates))]
    pub hostnames: Vec<String>,
}

fn fetch_hostname_candidates() -> Vec<CompletionCandidate> {
    match fetch_hostnames() {
        Ok(hostnames) => hostnames
            .into_iter()
            .map(CompletionCandidate::new)
            .collect(),
        Err(err) => {
            debug!("hostname completion unavailable: {err:?}");
            Vec::new()
        }
    }
}

fn fetch_hostnames() -> Result<Vec<String>> {
    let backend = backend::detect();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(anyhow::Error::from)?
        .block_on(backend.list_machines())
}
