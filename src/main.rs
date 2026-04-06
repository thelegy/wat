use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "wat")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run "Boot" + "Test"
    Switch(CommonBuildArgs),

    Boot(CommonBuildArgs),

    /// Run "Boot" and then perfrom reboot to activate
    Reboot(CommonBuildArgs),

    Test(CommonBuildArgs),

    DryActivate(CommonBuildArgs),

    /// Just build machine and print store path
    Build(CommonBuildArgs),

    Iso(CommonBuildArgs),

    Sdcard(CommonBuildArgs),
}

#[derive(Debug, Args)]
struct CommonBuildArgs {
    /// The name of the host to operate on
    hostname: String,
}

fn main() {
    let args = Cli::parse();
    println!("Hello, world!");
    println!("Args: {args:?}");
}
