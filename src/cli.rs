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
    /// Replay a recorded run without executing any commands
    #[command(hide = true)]
    Replay { path: std::path::PathBuf },

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
    Build(BuildArgs),

    /// Build ISO image and print store path
    Iso(CommonBuildArgs),

    /// Build SD card image and print store path
    Sdcard(CommonBuildArgs),
}

#[derive(Debug, Args)]
pub struct BuildArgs {
    #[command(flatten)]
    pub common: CommonBuildArgs,

    /// Atomically write a versioned JSON build report to this path
    #[arg(long)]
    pub results_file: Option<std::path::PathBuf>,
}

#[derive(Debug, Args)]
pub struct CommonBuildArgs {
    /// The names of the hosts to operate on
    #[arg(required_unless_present = "all", conflicts_with = "all", num_args = 1.., add = ArgValueCandidates::new(fetch_hostname_candidates))]
    pub hostnames: Vec<String>,

    /// Operate on all hosts in the flake
    #[arg(short = 'A', long)]
    pub all: bool,

    /// Stop starting deployments after a host fails; let running operations finish
    #[arg(long)]
    pub fail_early: bool,
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

impl Command {
    pub fn build_args_mut(&mut self) -> Option<&mut CommonBuildArgs> {
        match self {
            Self::Replay { .. } => None,
            Self::Build(args) => Some(&mut args.common),
            Self::Switch(args)
            | Self::Boot(args)
            | Self::Reboot(args)
            | Self::Test(args)
            | Self::DryActivate(args)
            | Self::Iso(args)
            | Self::Sdcard(args) => Some(args),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_file_is_build_only() {
        assert!(Cli::try_parse_from(["wat", "build", "--all", "--results-file", "report.json"]).is_ok());
        assert!(Cli::try_parse_from(["wat", "switch", "host", "--results-file", "report.json"]).is_err());
    }

    #[test]
    fn host_selection_requires_names_or_all_for_every_command() {
        for command in ["switch", "boot", "reboot", "test", "dry-activate", "build", "iso", "sdcard"] {
            assert!(Cli::try_parse_from(["wat", command]).is_err());
            for flag in ["-A", "--all"] {
                let mut cli = Cli::try_parse_from(["wat", command, flag]).unwrap();
                let args = cli.command.build_args_mut().unwrap();
                assert!(args.all);
                assert!(args.hostnames.is_empty());
                assert!(Cli::try_parse_from(["wat", command, flag, "host"]).is_err());
                assert!(Cli::try_parse_from(["wat", command, "host", flag]).is_err());
            }
            let mut cli = Cli::try_parse_from(["wat", command, "one", "two"]).unwrap();
            let args = cli.command.build_args_mut().unwrap();
            assert!(!args.all);
            assert_eq!(args.hostnames, ["one", "two"]);
        }
    }
}
