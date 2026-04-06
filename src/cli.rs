use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use clap_complete::{ArgValueCandidates, Shell, engine::CompletionCandidate};
use log::debug;

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
    /// The name of the host to operate on
    #[arg(add = ArgValueCandidates::new(fetch_hostname_candidates))]
    pub hostname: String,
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    /// The shell to generate completion scripts for
    pub shell: Shell,
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
    let binary = std::env::var("WAT_NIX_BINARY").unwrap_or_else(|_| "nix".to_string());
    let output = std::process::Command::new(&binary)
        .args([
            "eval",
            "--json",
            "--apply",
            "builtins.attrNames",
            ".#nixosConfigurations",
        ])
        .output()
        .with_context(|| format!("failed to invoke {binary} for hostname completion"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{binary} exited with status {}: {}",
            output.status,
            stderr.trim()
        );
    }

    let stdout =
        String::from_utf8(output.stdout).context("hostname output contained invalid UTF-8")?;
    let mut hostnames: Vec<String> =
        serde_json::from_str(&stdout).context("failed to parse hostname list")?;
    hostnames.sort();
    Ok(hostnames)
}
