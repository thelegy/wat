use clap::{Args, Parser, Subcommand};

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
    pub hostname: String,
}
