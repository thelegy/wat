use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

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

    /// Generate shell completion scripts
    Completion(CompletionArgs),
}

#[derive(Debug, Args)]
pub struct CommonBuildArgs {
    /// The name of the host to operate on
    pub hostname: String,
}

#[derive(Debug, Args)]
pub struct CompletionArgs {
    /// The shell to generate completion scripts for
    pub shell: Shell,
}

pub fn print_completions(shell: Shell) {
    let mut command = Cli::command();
    let name = command.get_name().to_string();
    clap_complete::generate(shell, &mut command, name, &mut std::io::stdout());
}
